# Semantic Analyzer エラーハンドリング改善

## 概要

`src/semantic_analyzer/mod.rs` において、現在 `panic!` でエラーを返している箇所を `Result` 型でエラーを返すように変更する設計。

## 現状分析

### 現在の panic! 使用箇所

| 箇所 | エラー内容 | 行 |
|------|-----------|-----|
| `add_identifier` | 識別子の重複定義 | 156行目 |
| `analyze_internal` | ブロックスコープ変数（未実装） | 187行目 |
| `analyze_internal` | グローバル変数（未実装） | 191行目 |
| `analyze_internal` | ネストした関数宣言（未対応） | 203行目 |
| `analyze_internal` | ルートレベルでの return 文 | 225行目 |
| `analyze_internal` | ルートレベルでの式文 | 231行目 |
| `analyze_internal` | ルートレベルでの continue 文 | 237行目 |
| `analyze_internal` | ルートレベルでの break 文 | 243行目 |

### 既存のエラーハンドリング実装

#### token_parser

```rust
// base/mod.rs
pub struct CodeParseError {
    pub code_pointer: Option<usize>,  // ソースコード上のバイト位置
    pub message: String,
}

// token_parser/mod.rs
pub struct TokenInfo {
    pub code_pointer: usize,
}
pub type PrettyToken = (Token, TokenInfo);
```

- 各トークンが `TokenInfo` で `code_pointer`（バイト位置）を保持
- エラー時に `CodeParseError` を返却
- `lib.rs` 経由で `Result<T, Vec<CodeParseError>>` として返却

#### tree_parser

```rust
// tree_parser/statement/mod.rs
pub enum Statement {
    VariableDeclaration(String, Box<Expression>),
    FunctionDeclaration(String, Vec<String>, Vec<Statement>),
    // ... 他のバリアント
    Invalid(usize),  // エラー情報のインデックス
}
```

- `Statement` と `Expression` は `Invalid` バリアントでエラーを表現
- エラーは `Vec<CodeParseError>` に蓄積し、最後にまとめて返却
- **問題点**: `Statement`、`Expression` 自体は `code_pointer` 情報を持っていない

## 設計方針

### 選択肢1: Statement に位置情報を追加する（推奨）

#### 概要

`Statement` と `Expression` に位置情報を付与し、semantic_analyzer でその情報を利用する。

#### 変更内容

1. **tree_parser の Statement/Expression に位置情報を追加**

```rust
// 案A: タプル形式
pub type LocatedStatement = (Statement, SourceLocation);
pub type LocatedExpression = (Expression, SourceLocation);

// 案B: 構造体形式（推奨）
pub struct LocatedStatement {
    pub statement: Statement,
    pub location: SourceLocation,
}

pub struct LocatedExpression {
    pub expression: Expression,
    pub location: SourceLocation,
}

pub struct SourceLocation {
    pub start: usize,  // 開始バイト位置
    pub end: usize,    // 終了バイト位置（オプション）
}
```

2. **semantic_analyzer に専用のエラー型を定義**

```rust
pub struct SemanticError {
    pub code_pointer: Option<usize>,
    pub message: String,
}

// または CodeParseError を再利用（推奨、シンプル）
```

3. **analyze 関数の返り値を Result に変更**

```rust
// 現在
pub fn analyze(root: &Vec<Statement>) -> Scope

// 変更後
pub fn analyze(root: &Vec<Statement>) -> Result<Scope, Vec<CodeParseError>>
```

4. **内部関数も Result を返すように変更**

```rust
fn analyze_internal(
    statements: &Vec<Statement>,
    scope_type: ScopeType,
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>>
```

#### メリット

- エラー時に正確な位置情報を提供可能
- ユーザーへのエラーメッセージが充実
- 他のパーサー (token_parser, tree_parser) と一貫したエラーハンドリング

#### デメリット

- tree_parser への変更が必要（影響範囲が広い）
- Statement/Expression の全バリアントに位置情報を付与する作業量

### 選択肢2: Statement に位置情報を追加せず、エラーメッセージのみ返す

#### 概要

位置情報なしで `CodeParseError { code_pointer: None, message }` を返す。

#### 変更内容

```rust
pub fn analyze(root: &Vec<Statement>) -> Result<Scope, Vec<CodeParseError>>
```

エラー発生時:
```rust
return Err(vec![code_parse_error!(
    "semantic error: the name is already used".to_string()
)]);
```

#### メリット

- 変更が semantic_analyzer のみで完結
- 実装が容易

#### デメリット

- 位置情報がないため、ユーザーはエラー箇所を特定しにくい

## 推奨する実装方針

**段階的なアプローチ（選択肢2 → 選択肢1）**

### Phase 1: Result 型への移行（選択肢2）

1. `semantic_analyzer` を `Result<Scope, Vec<CodeParseError>>` を返すように変更
2. `code_pointer: None` でエラーを返す
3. 呼び出し元（`lib.rs`, `bin/nospace20.rs`）を修正

### Phase 2: 位置情報の付与（選択肢1）

1. `Statement`、`Expression` に位置情報を追加
2. `semantic_analyzer` で位置情報を活用

### Phase 1 の実装詳細

#### 1. semantic_analyzer/mod.rs の変更

```rust
use crate::{base::CodeParseError, code_parse_error};

// analyze_internal の返り値を Result に変更
fn analyze_internal(
    statements: &Vec<Statement>,
    scope_type: ScopeType,
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    // ...
    // panic! を Err に置換
    if self.identifier_map.contains_key(&name) {
        return Err(vec![code_parse_error!(
            "semantic error: the name is already used".to_string()
        )]);
    }
    // ...
}

// analyze の返り値を Result に変更
pub fn analyze(root: &Vec<Statement>) -> Result<Scope, Vec<CodeParseError>> {
    analyze_internal(root, ScopeType::Root)
        .map(|(scope, _)| scope.build())
}
```

#### 2. lib.rs の変更

```rust
pub fn syntactic_analyze(root: &Vec<Statement>) -> Result<Scope, Vec<CodeParseError>> {
    semantic_analyzer::analyze(root)
}
```

#### 3. bin/nospace20.rs の変更

```rust
let a = handle_parse_error(syntactic_analyze(&s), &text);
```

## 懸念点の検討

### Q1: token_parser、tree_parser の実装を応用できるか？

**回答: 一部可能、ただし制限あり**

- `CodeParseError` 型と `code_parse_error!` マクロは再利用可能
- 位置情報（`code_pointer`）の取得には tree_parser の変更が必要
  - 現在の `Statement` は位置情報を持っていない
  - tree_parser 側で `TokenInfo` から位置情報を引き継ぐ必要がある

### Q2: 他に懸念点はあるか？

**検討した結果、以下の点に注意が必要：**

1. **エラーの蓄積 vs 即時リターン**
   - tree_parser は複数のエラーを蓄積してまとめて返す設計
   - semantic_analyzer も同様にすべきか、最初のエラーで即時リターンすべきか
   - **推奨**: Phase 1 では即時リターン（シンプル）、将来的に蓄積を検討

2. **convert_to_exec_expression のエラーハンドリング**
   - 現在 `Expression::Invalid` は `unreachable!` としている
   - 正常系では到達しないはずだが、念のため `Err` を返す設計にすべきか
   - **推奨**: `unreachable!` のままで問題ない（パース成功後のみ呼ばれるため）

3. **内部関数の再帰呼び出し**
   - `analyze_internal` は再帰的に呼び出される
   - `Result` を返す場合、`?` 演算子でエラーを伝播させる
   - ネストした呼び出しでもエラーが正しく伝播することを確認する

4. **未実装機能のエラー種別**
   - 「未実装」と「意味エラー」を区別すべきか
   - **推奨**: 現時点では同一の `CodeParseError` で対応（シンプル）

## 作業見積もり

| フェーズ | 作業内容 | 見積もり |
|---------|---------|---------|
| Phase 1 | Result 型への移行 | 1-2時間 |
| Phase 2 | 位置情報の追加（tree_parser含む） | 3-5時間 |

## 結論

1. **Phase 1 から着手**し、まず `panic!` を `Result` に置き換える
2. **位置情報なし**でも機能するため、ユーザー体験は大幅に向上
3. **Phase 2 は必要に応じて**実施（ユーザーからのフィードバック次第）

懸念点は特になく、既存の `CodeParseError` インフラを活用することで、一貫性のあるエラーハンドリングが実現可能。
