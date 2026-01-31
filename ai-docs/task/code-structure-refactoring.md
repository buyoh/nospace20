# ソースコード構成のリファクタリング

## 概要

現在のソースコード構成における問題点を整理し、リファクタリングの計画を立てる。
主な問題は型の重複、マクロの重複、エラーハンドリングの不統一など。

## 問題点リスト

### 🔴 優先度: 高

#### 1. 型の重複定義 (`Expression` vs `ExecExpression`)

**現状**:
- `tree_parser/expression.rs`: `Expression`, `Statement` (パース結果)
- `syntactic_analyzer/mod.rs`: `ExecExpression`, `ExecStatement` (実行用)
- `convert_to_exec_expression()` で変換

**役割の違い** (意図的な設計):
- `Expression`/`Statement`: 構文解析の結果のみを保持。`Invalid` バリアントを持つ
- `ExecExpression`/`ExecStatement`: 意味解析後の実行可能な表現。`Invalid` を持たない
  - 将来的にはスコープ解決済みの識別子情報を保持予定
  - 宣言文は `Scope` 構造に変換されるため `ExecStatement` には含まれない

**現状の問題**:
- 将来の拡張を見越した設計だが、現時点では構造が類似
- 意味解析の機能拡張 (変数名解決等) が進めば差異が明確になる予定

**改善案**:
- [x] ~~Option A: パース結果を `Result` で表現~~ → 現状維持
- [x] ~~Option B: `ExecStatement`/`ExecExpression` を削除~~ → 役割が異なるため不採用
- [x] Option C: 意味解析の機能拡張に合わせて差異を明確化 (採用)

**関連イシュー**: なし

---

#### 2. `Operator1`/`Operator2` の不要な `Clone` derive

**現状**:
```rust
#[derive(Clone)] // TODO: REMOVE
pub enum Operator2 { ... }
```

**問題**:
- `syntactic_analyzer` で参照からのコピーを行っている
- 本来は所有権の移動で十分

**影響範囲**:
- `src/tree_parser/expression.rs` (L43-62)
- `src/syntactic_analyzer/mod.rs` (convert_to_exec_expression)

**改善案**:
- [ ] `convert_to_exec_expression` を所有権移動バージョンに変更
- [ ] `Clone` derive を削除

**関連イシュー**: なし

---

#### 3. マクロの重複 (`match_expect_token!`)

**現状**:
- `src/tree_parser/expression.rs` (L16-30)
- `src/tree_parser/statement.rs` (L12-49)
- 完全に同じマクロが2ファイルに存在

**問題**:
- メンテナンス時に両方修正が必要
- コードの一貫性が保てない

**影響範囲**:
- `src/tree_parser/expression.rs`
- `src/tree_parser/statement.rs`

**改善案**:
- [ ] `src/tree_parser/macros.rs` に共通マクロを抽出
- [ ] または `src/tree_parser/mod.rs` 内でマクロ定義

**関連イシュー**: なし

---

### 🟡 優先度: 中

#### 4. モジュール可視性の不整合

**現状**:
- `tree_parser`: `pub(crate) use` で公開
- `syntactic_analyzer`: `pub` で直接公開
- `interpreter`: 内部型は非公開

**問題**:
- 一貫性がなく、意図が不明確
- API設計方針が定まっていない

**影響範囲**:
- `src/tree_parser/mod.rs`
- `src/syntactic_analyzer/mod.rs`
- `src/interpreter/mod.rs`

**改善案**:
- [ ] 公開APIのポリシーを明確化
- [ ] `lib.rs` で必要な型のみ再公開

**関連イシュー**: なし

---

#### 5. エラー型の二重構造

**現状**:
```rust
pub struct CodeParseErrorInternal {
    pub code_pointer: Option<usize>,
    pub message: String,
    pub internal_line: u32,      // デバッグ用
    pub internal_file: &'static str,  // デバッグ用
}

pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: String,
}
```

**問題**:
- `shrink()` メソッドで都度変換が必要
- `internal_line`/`internal_file` が正しく機能していない
  - `code_parse_error!` マクロがヘルパー関数内で展開されるため、実際のエラー発生箇所ではなくヘルパー関数の行を指す
  - 修正には周辺の実装変更が必要 (呼び出し元で `line!()` を評価して渡す)

**影響範囲**:
- `src/base/mod.rs`
- 全パーサーモジュール

**改善案**:
- [ ] デバッグ情報の実装コストが高いため削除を検討 (`internal_line`, `internal_file` の削除)
- [ ] `CodeParseErrorInternal` と `CodeParseError` の統合

**関連イシュー**: なし

---

#### 6. `syntactic_analyzer` のエラーハンドリング不足

**現状の panic! 分類結果**:

| 分類 | 箇所 | メッセージ | 対応 |
|------|------|------------|------|
| 未実装機能 | L159 | `"todo: block scoped variable is not implemented"` | panic! が適切 ✅ |
| 未実装機能 | L162 | `"todo: global variable is not implemented"` | panic! が適切 ✅ |
| ユーザーエラー | L176 | `"semantic error: nested function declaration is not supported"` | TODO: Result型に変更 |
| ユーザーエラー | L197 | `"semantic error: return statement outside of function"` | TODO: Result型に変更 |
| ユーザーエラー | L201 | `"semantic error: expression statement at root level"` | TODO: Result型に変更 |
| ユーザーエラー | L206 | `"semantic error: continue statement outside of function"` | TODO: Result型に変更 |
| ユーザーエラー | L211 | `"semantic error: break statement outside of function"` | TODO: Result型に変更 |
| 到達不能 | L66 | `Expression::Invalid` 分岐 | `unreachable!` に変更済 ✅ |

**影響範囲**:
- `src/syntactic_analyzer/mod.rs`
- `src/interpreter/mod.rs` (変数参照など)

**改善案**:
- [ ] `SemanticError` 型を定義
- [ ] `syntactic_analyze` を `Result<Scope, Vec<SemanticError>>` に変更
- [ ] スコープ検証、変数解決を事前チェック

**関連イシュー**: なし

---

#### 7. 未実装の `compiler` モジュール

**現状**:
- `src/compiler/mod.rs`: `// todo!` のみ
- `src/compiler/grayspace/`: 空ディレクトリ

**問題**:
- 使用されていないコードが残っている
- プロジェクト構造が不明瞭

**影響範囲**:
- `src/compiler/` ディレクトリ全体

**改善案**:
- [ ] 将来実装予定なら TODO コメントを追加
- [ ] 不要なら削除

**関連イシュー**: なし

---

### 🟢 優先度: 低 (改善提案)

#### 8. コメント・命名の修正

| 箇所 | 問題 | 修正案 |
|------|------|--------|
| `token_parser/mod.rs:44` | `Colon` のコメントが `// ;` | `// :` に修正 |
| `syntactic_analyzer` | 名前が "syntactic" だが実際は semantic analyzer | `semantic_analyzer` にリネーム |
| 複数箇所 | `stat`, `e`, `f` など短い変数名 | より説明的な名前に変更 |

**関連イシュー**: なし

---

## 推奨リファクタリング順序

### Phase 1: 早期改善 (即座に実施可能)

1. ✅ **マクロの重複解消** (問題3)
   - リスク: 低
   - 効果: コードの保守性向上
   - 所要時間: 30分

2. ✅ **コメント・命名の修正** (問題8)
   - リスク: 低
   - 効果: 可読性向上
   - 所要時間: 30分

### Phase 2: 中期改善 (設計検討が必要)

3. 🔲 **エラー型の統一** (問題5)
   - リスク: 中
   - 効果: エラーハンドリングの一貫性
   - 所要時間: 2-3時間
   - 依存: なし

4. 🔲 **`syntactic_analyzer` のエラーハンドリング** (問題6)
   - リスク: 中
   - 効果: エラー報告の改善
   - 所要時間: 4-6時間
   - 依存: Phase 2-3 (エラー型統一)

### Phase 3: 大規模リファクタリング (アーキテクチャ変更)

5. 🔲 **型の重複定義の解消** (問題1)
   - リスク: 高
   - 効果: コードベースの簡素化
   - 所要時間: 1-2日
   - 依存: Phase 2-3, 2-4 (エラーハンドリング整備後)
   - 注意: 設計方針の確定が必要

6. 🔲 **`Clone` の削除** (問題2)
   - リスク: 中
   - 効果: パフォーマンス向上
   - 所要時間: 2-3時間
   - 依存: Phase 3-5 (型統合後に自然に解決する可能性)

### Phase 4: クリーンアップ

7. 🔲 **未使用コードの削除** (問題7)
   - リスク: 低
   - 効果: プロジェクト構造の明確化
   - 所要時間: 30分

8. 🔲 **モジュール可視性の統一** (問題4)
   - リスク: 低
   - 効果: API設計の明確化
   - 所要時間: 1時間

---

## 設計決定が必要な事項

### 1. Expression vs ExecExpression の統合方針

**決定**: Option C 採用 (現状維持 + 段階的拡張)

**理由**:
- `Expression`: 構文解析結果のみを保持 (`Invalid` バリアント含む)
- `ExecExpression`: 実行可能な表現 (スコープ解決済み識別子を保持予定)
- 意味解析機能の拡張に伴い差異が明確になる

**次のステップ**:
- `ExecExpression::Variable(String)` を `ExecExpression::Variable(IdentifierInfo)` に変更
- ソースコードに設計意図をコメントとして追加済み

---

### 2. エラー型の設計方針

**決定**: Option B 採用 (デバッグ情報を削除)

**理由**:
- `internal_line`/`internal_file` の実装には周辺コードの大幅な変更が必要
  - 各ヘルパー関数に `line!()` 結果を引数として渡す必要がある
  - マクロ内で `line!()` を評価するとマクロ定義位置ではなく展開位置の行が得られるが、
    ヘルパー関数内でマクロが展開される現状の設計では意味をなさない
- 実装コストに見合わない

**次のステップ**:
- [ ] `CodeParseErrorInternal` から `internal_line`, `internal_file` を削除
- [ ] `CodeParseErrorInternal` と `CodeParseError` を統合

---

### 3. モジュール名の変更

**現状**: `syntactic_analyzer`
**候補**: `semantic_analyzer`

**理由**:
- スコープ解析、識別子解決は意味解析 (semantic analysis) の範疇
- 構文解析 (syntactic analysis) は `tree_parser` が担当

**影響**:
- モジュール名変更
- ディレクトリ名変更
- import文の更新
- ドキュメントの更新

**決定**: 未定（要合意）

---

## 関連ドキュメント

- [architecture/overview.md](../architecture/overview.md) - システム概要
- [architecture/modules.md](../architecture/modules.md) - モジュール詳細
- [test-error-handling.md](test-error-handling.md) - テストのエラーハンドリング

---

## 進捗記録

### 2026-01-31

- ✅ ソースコード構成のレビュー完了
- ✅ 問題点の整理と優先度付け
- ✅ 設計方針の決定
  - Expression vs ExecExpression: 現状維持 (役割が異なる)
  - エラー型: デバッグ情報を削除する方針
  - panic! の分類完了
- ✅ ソースコードへのコメント追加
  - `ExecExpression`/`ExecStatement` に役割を説明するドキュメントコメント追加
  - `Expression::Invalid` 分岐を `unreachable!` に変更
  - 各 `panic!` に TODO コメント追加 (未実装 or Result型に変更すべき)
  - `CodeParseErrorInternal` のフィールドに問題点をドキュメント化
- 🔲 リファクタリング実施待ち

---

## メモ

- 大規模リファクタリング前に、既存テストが全てパスすることを確認
- 各Phaseごとに全テストを実行して回帰を防ぐ
- `semantic_analyzer` への名前変更は影響範囲が大きいため、他の改善と分離して実施
