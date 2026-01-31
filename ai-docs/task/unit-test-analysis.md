# ユニットテスト分析レポート

## 概要

本ドキュメントは、nospace20 プロジェクトにおけるユニットテストの現状と、テスト追加容易性について分析した結果をまとめたものである。

---

## 1. 現在のユニットテスト状況

### 1.1 モジュール別テスト有無

| モジュール | ファイル | ユニットテスト | 備考 |
|-----------|----------|---------------|------|
| token_parser | `mod.rs`, `test.rs` | **あり** (3件) | 数値・識別子・複合式のテスト |
| tree_parser | `mod.rs`, `expression.rs`, `statement.rs` | **なし** | マクロのみ存在 |
| semantic_analyzer | `mod.rs` | **なし** | |
| interpreter | `mod.rs` | **なし** | |
| compiler | `mod.rs`, `grayspace/` | **なし** | 未実装（`todo!` のみ） |
| base | `mod.rs` | **なし** | マクロのみ、小規模 |
| logger | `mod.rs` | **なし** | ユーティリティ関数 |

### 1.2 テストの種類

現在のテスト構成:

```
src/token_parser/test.rs  ... ユニットテスト (3件)
tests/code_test.rs        ... 統合テスト (約50件)
```

- **ユニットテスト**: `#[cfg(test)]` モジュール内、token_parser のみ
- **統合テスト (large テスト)**: `tests/code_test.rs` で `.ns` ファイルを読み込み、パース→解析→実行のパイプライン全体をテスト

---

## 2. テスト追加容易性の分析

### 2.1 token_parser (テスト追加: **容易**)

**現状**:
- 独立した `test.rs` ファイルが存在
- マクロ (`test_ok_parse_single!`, `test_ok_parse!`) を使って簡潔にテストを記述可能
- 内部関数 `parse_to_tokens_internal` が `pub(crate)` でテストからアクセス可能

**追加方法**:
```rust
test_ok_parse_single!(test_name, "input", Token::Expected);
test_ok_parse!(test_name, "input", it => { assert_matches!(it.next(), ...); });
```

**課題**:
- エラーケースのテストが不足（TODOコメントあり）
- `TokenInfo` のテストが不足（位置情報の検証）

### 2.2 tree_parser (テスト追加: **やや困難**)

**現状**:
- ユニットテストなし
- `ExpressionBuilder` および `StatementBuilder` は private struct
- 公開インターフェースは `parse_to_tree()` のみ

**課題**:
1. **内部構造へのアクセス不可**: `ExpressionBuilder::parse()` や `StatementBuilder::parse()` は private
2. **テスト用ヘルパーなし**: トークン列を簡単に生成するヘルパーがない

**推奨される設計変更**:
```rust
// Option A: pub(crate) での公開
pub(crate) fn parse_to_expression_tree_root(...) -> ...

// Option B: テスト専用モジュール
#[cfg(test)]
mod test {
    use super::*;
    
    fn tokens_from_str(s: &str) -> Vec<PrettyToken> {
        crate::token_parser::parse_to_tokens(s).unwrap()
    }
    
    #[test]
    fn test_parse_simple_expression() {
        let tokens = tokens_from_str("1 + 2");
        let (expr, errs) = parse_to_expression_tree_root(&mut tokens.iter().peekable());
        assert!(errs.is_empty());
        // ...
    }
}
```

### 2.3 semantic_analyzer (テスト追加: **やや困難**)

**現状**:
- ユニットテストなし
- `analyze()` は公開されているが、内部の `analyze_internal()` と `ScopeBuilder` は private
- テスト時に `Statement` を手動構築する必要がある

**課題**:
1. **Statement の手動構築が煩雑**: `Statement::FunctionDeclaration(...)` 等を手作りする必要
2. **中間結果の検証困難**: `Scope` の内部構造は一部 private

**推奨される設計変更**:
```rust
// テスト用ビルダーの追加
#[cfg(test)]
mod test_helpers {
    pub fn make_simple_function(name: &str, body: Vec<Statement>) -> Statement {
        Statement::FunctionDeclaration(name.to_string(), vec![], body)
    }
}

// または tree_parser との統合テスト用ヘルパー
#[cfg(test)]
fn analyze_code(code: &str) -> Scope {
    let tokens = crate::token_parser::parse_to_tokens(code).unwrap();
    let tree = crate::tree_parser::parse_to_tree(&tokens).unwrap();
    crate::semantic_analyzer::analyze(&tree)
}
```

### 2.4 interpreter (テスト追加: **やや困難**)

**現状**:
- ユニットテストなし
- `LocalEnvironment` は private struct
- 公開インターフェースは `interpret_func()` のみ

**課題**:
1. **個別関数のテスト困難**: `interpret_expression()`, `interpret_statement()` 等は private
2. **Environment のモック困難**: stdin/stdout のモックは `new_with_buffers()` で可能だが、テスト用ヘルパーがない

**推奨される設計変更**:
```rust
// lib.rs に既にテスト用関数が存在
pub fn interpret_func_testing(scope: &Scope, func_name: &str) -> BTreeMap<i64, i64>
pub fn interpret_func_with_io(scope: &Scope, func_name: &str, stdin: &str) -> (BTreeMap<i64, i64>, String)

// モジュール内テスト追加時
#[cfg(test)]
mod test {
    fn run_code(code: &str) -> Option<i64> {
        let tokens = crate::parse_to_tokens(&code.to_string()).unwrap();
        let tree = crate::parse_to_tree(&tokens).unwrap();
        let scope = crate::syntactic_analyze(&tree);
        crate::interpret_func(&scope, "main")
    }
}
```

### 2.5 logger (テスト追加: **容易**)

**現状**:
- ユニットテストなし
- 純粋関数のみ（副作用なし）
- 公開 API: `TextCode::new()`, `line()`, `char_index_to_line()`

**追加方法**: テストを直接追加可能

```rust
#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn test_char_index_to_line() {
        let code = TextCode::new("abc\ndef\nghi");
        assert_eq!(code.char_index_to_line(0), (0, 0));
        assert_eq!(code.char_index_to_line(4), (1, 0));
    }
}
```

### 2.6 base (テスト追加: **不要**)

- マクロ `code_parse_error!` のみ
- 他モジュールのテストで間接的にテストされる
- 単体テストの必要性は低い

---

## 3. ユニットテスト数の評価

### 3.1 現状

| カテゴリ | 件数 |
|---------|------|
| ユニットテスト (token_parser) | 3件 |
| 統合テスト (code_test.rs) | 約50件 |

### 3.2 推奨されるテスト追加

| モジュール | 推奨追加数 | 優先度 | 理由 |
|-----------|-----------|--------|------|
| token_parser | +10〜15件 | 高 | エラーケース、エスケープシーケンス、エッジケース |
| tree_parser | +15〜20件 | 高 | 式・文のパース、エラー復帰 |
| semantic_analyzer | +10件 | 中 | スコープ解決、エラーケース |
| interpreter | +10件 | 中 | 演算子、組み込み関数、制御フロー |
| logger | +5件 | 低 | 行番号変換、エッジケース |

### 3.3 テストカバレッジの観点

現在の統合テストは E2E（End-to-End）テストとして機能しているが、以下の観点が不足:

1. **境界値テスト**: 最大値、空入力、特殊文字
2. **エラーパステスト**: 不正入力時の挙動
3. **回帰テスト**: バグ修正時の再発防止

---

## 4. 改善提案

### 4.1 短期的改善 (設計変更なし)

1. **token_parser のテスト拡充**
   - エラーケースのテスト追加
   - 文字リテラル、エスケープシーケンスのテスト
   - コメントのテスト

2. **logger のテスト追加**
   - 純粋関数なので容易に追加可能

### 4.2 中期的改善 (軽微な設計変更)

1. **テスト用ヘルパー関数の追加**
   ```rust
   // lib.rs または各モジュールに追加
   #[cfg(test)]
   pub mod test_helpers {
       pub fn parse_expression(code: &str) -> Box<Expression> { ... }
       pub fn parse_statements(code: &str) -> Vec<Statement> { ... }
   }
   ```

2. **内部関数の `pub(crate)` 化**
   - `tree_parser::parse_to_expression_tree_root`
   - `semantic_analyzer::analyze_internal`

### 4.3 長期的改善 (アーキテクチャ変更)

1. **テスト可能な設計パターンの導入**
   - Builder パターンのテスト用公開
   - Visitor パターンによる AST 走査のテスト容易化

2. **プロパティベーステスト**
   - `proptest` や `quickcheck` クレートの導入
   - 「パース→再構築→再パース = 元のAST」の検証

---

## 5. 結論

### テスト追加容易性サマリー

| モジュール | 現状の容易性 | 改善後の容易性 |
|-----------|-------------|---------------|
| token_parser | ◎ 容易 | ◎ 容易 |
| tree_parser | △ やや困難 | ○ 容易（ヘルパー追加後） |
| semantic_analyzer | △ やや困難 | ◎ 容易（サブモジュール分割後） |
| interpreter | △ やや困難 | ◎ 容易（サブモジュール分割後） |
| logger | ◎ 容易 | ◎ 容易 |
| base | - 不要 | - 不要 |
| compiler | - 未実装 | - |

### テスト数の評価

- **現状**: ユニットテスト 3件 → **不十分**
- **推奨**: 各モジュールに最低 5〜10件のユニットテスト
- **優先順位**: token_parser > tree_parser > semantic_analyzer = interpreter > logger

### 推奨アプローチ

1. **サブモジュール分割 + `pub(crate)`** が最も効果的
2. interpreter と semantic_analyzer は組み込み関数・変換処理を分離することで大幅改善
3. 詳細は「6. サブモジュール分割によるテスト容易性向上」および「7. 改善タスク一覧」を参照

---

## 6. サブモジュール分割によるテスト容易性向上

### 6.1 分割が有効なケース

現在テストが困難な理由は「モジュールの大きさ」ではなく、**private な構造体・関数へのアクセス不可**が原因である。
サブモジュールに分割し、適切な可視性（`pub(crate)`）を設定することで、テスト容易性が向上する。

| 分割パターン | 効果 | 例 |
|-------------|------|-----|
| 型定義の分離 | ○ 有効 | `types.rs` に `Flow`, `ExpressionFlow` を分離 |
| 組み込み関数の分離 | ◎ 非常に有効 | `builtins.rs` に `__trace`, `__puti` 等を分離 |
| 純粋関数の分離 | ◎ 非常に有効 | 演算処理を `operations.rs` に分離 |

### 6.2 分割が効果薄のケース

| ケース | 理由 |
|--------|------|
| 単に private のまま分割 | 可視性が変わらないためテストできない |
| 密結合な処理の分割 | 分割しても相互依存が残り、単体テスト困難 |

### 6.3 推奨されるモジュール構造

#### interpreter

```
interpreter/
├── mod.rs           # 公開 API (interpret_func)
├── environment.rs   # Environment の定義と実装
├── builtins.rs      # 組み込み関数 (__trace, __puti, __geti, __getc, __putc)
├── operations.rs    # 演算処理 (bool_to_int, 二項演算等)
└── test.rs          # ユニットテスト
```

**分割のポイント**:
- `builtins.rs`: 各組み込み関数を `pub(crate)` で公開し、個別テスト可能に
- `operations.rs`: 純粋関数として分離し、副作用なしでテスト可能に

#### semantic_analyzer

```
semantic_analyzer/
├── mod.rs           # 公開 API (analyze)
├── types.rs         # ExecExpression, ExecStatement, Scope, Function, Variable
├── scope_builder.rs # ScopeBuilder
├── converter.rs     # convert_to_exec_expression, convert_to_exec_statement
└── test.rs          # ユニットテスト
```

**分割のポイント**:
- `types.rs`: 型定義を分離し、テストで構造体を直接構築可能に
- `converter.rs`: 変換関数を `pub(crate)` で公開し、個別テスト可能に

### 6.4 可視性設計の指針

```rust
// builtins.rs
pub(crate) fn builtin_trace(env: &mut Environment, key: i64) -> i64 {
    if let Some(v) = env.traced.get_mut(&key) {
        *v += 1;
    } else {
        env.traced.insert(key, 1);
    }
    0
}

// test.rs
#[cfg(test)]
mod test {
    use super::builtins::builtin_trace;
    use super::Environment;

    #[test]
    fn test_builtin_trace() {
        let mut env = Environment::new();
        builtin_trace(&mut env, 42);
        assert_eq!(env.traced.get(&42), Some(&1));
        builtin_trace(&mut env, 42);
        assert_eq!(env.traced.get(&42), Some(&2));
    }
}
```

---

## 7. 改善タスク一覧

### Phase 1: 即時対応（設計変更なし）

- [ ] **T1-1**: token_parser のエラーケーステスト追加（5件程度）
- [ ] **T1-2**: token_parser の文字リテラル・エスケープシーケンステスト追加（5件程度）
- [ ] **T1-3**: logger のユニットテスト追加（3〜5件）

### Phase 2: サブモジュール分割

- [ ] **T2-1**: interpreter を builtins.rs, operations.rs に分割
  - `builtins.rs`: `__trace`, `__assert`, `__puti`, `__putc`, `__geti`, `__getc`
  - `operations.rs`: `bool_to_int`, 二項演算の評価処理
  - 各関数を `pub(crate)` で公開
- [ ] **T2-2**: semantic_analyzer を types.rs, converter.rs に分割
  - `types.rs`: `ExecExpression`, `ExecStatement`, `Variable`, `Function`, `Scope`
  - `converter.rs`: `convert_to_exec_expression`, `convert_to_exec_statement`
  - 各関数・型を `pub(crate)` で公開
- [ ] **T2-3**: 分割後のモジュールにユニットテスト追加

### Phase 3: テストヘルパー整備

- [ ] **T3-1**: tree_parser 用ヘルパー関数追加
  ```rust
  #[cfg(test)]
  pub(crate) fn parse_expression_from_str(code: &str) -> Box<Expression>
  ```
- [ ] **T3-2**: semantic_analyzer 用ヘルパー関数追加
  ```rust
  #[cfg(test)]
  pub(crate) fn analyze_from_str(code: &str) -> Scope
  ```
- [ ] **T3-3**: interpreter 用ヘルパー関数追加
  ```rust
  #[cfg(test)]
  pub(crate) fn run_code(code: &str) -> Option<i64>
  ```

### Phase 4: テスト拡充

- [ ] **T4-1**: tree_parser のユニットテスト追加（10件程度）
- [ ] **T4-2**: semantic_analyzer のユニットテスト追加（10件程度）
- [ ] **T4-3**: interpreter のユニットテスト追加（10件程度）

---

## 付録: テスト追加のクイックスタート

### A. token_parser にテストを追加

[src/token_parser/test.rs](../../src/token_parser/test.rs) を編集:

```rust
// エラーケースのテスト例
#[test]
fn test_unclosed_char_literal() {
    let result = res_parse_to_tokens_internal(&mut to_iter("'a"));
    assert!(result.is_err());
}
```

### B. 新規モジュールにテストを追加

1. モジュールファイルの末尾に追加:
   ```rust
   #[cfg(test)]
   mod test {
       use super::*;
       
       #[test]
       fn test_example() {
           // ...
       }
   }
   ```

2. または、別ファイル `test.rs` を作成し、`mod.rs` で `#[cfg(test)] mod test;` を追加
