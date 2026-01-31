# semantic_analyzer ユニットテスト追加

## 概要

semantic_analyzer モジュールにユニットテストを追加するためのタスク。
現状ユニットテストがなく、追加には設計変更（サブモジュール分割）が必要。

## 背景

[unit-test-analysis.md](../done-task/unit-test-analysis.md) の分析結果より分離。

## 現状の課題

1. **Statement の手動構築が煩雑**: `Statement::FunctionDeclaration(...)` 等を手作りする必要
2. **中間結果の検証困難**: `Scope` の内部構造は一部 private
3. `analyze()` は公開されているが、内部の `analyze_internal()` と `ScopeBuilder` は private

## 改善タスク

### Phase 1: サブモジュール分割

- [ ] **T1-1**: semantic_analyzer を types.rs, converter.rs に分割
  - `types.rs`: `ExecExpression`, `ExecStatement`, `Variable`, `Function`, `Scope`
  - `converter.rs`: `convert_to_exec_expression`, `convert_to_exec_statement`
  - 各関数・型を `pub(crate)` で公開

### Phase 2: テストヘルパー整備

- [ ] **T2-1**: AST（Statement/Expression）を手動構築するためのビルダー追加
  ```rust
  #[cfg(test)]
  mod test_helpers {
      use super::*;
      
      pub fn make_number_expr(value: i64) -> Expression {
          Expression::Number(value)
      }
      pub fn make_variable_expr(name: &str) -> Expression {
          Expression::Variable(name.to_string())
      }
      pub fn make_binary_expr(op: &str, left: Expression, right: Expression) -> Expression {
          Expression::BinaryOperator(op.to_string(), Box::new(left), Box::new(right))
      }
      pub fn make_var_decl(name: &str, init: Expression) -> Statement {
          Statement::VariableDeclaration(name.to_string(), init)
      }
      pub fn make_function(name: &str, args: Vec<&str>, body: Vec<Statement>) -> Statement {
          Statement::FunctionDeclaration(
              name.to_string(),
              args.iter().map(|s| s.to_string()).collect(),
              body
          )
      }
  }
  ```
- [ ] **T2-2**: 外部ファイル（JSON/YAML）によるテストケース定義の検討
  - 複雑なASTを記述する場合、可読性のため外部ファイル化を検討
  - `resources/tests/unit/semantic_analyzer/` にテストデータを配置

### Phase 3: ユニットテスト追加

- [ ] **T3-1**: semantic_analyzer のユニットテスト追加（10件程度）
  - スコープ解決（ASTを手動構築）
  - 変数宣言
  - 関数定義
  - エラーケース

**注意**: ユニットテストでは `token_parser` / `tree_parser` に依存せず、AST を直接構築すること。
文字列からパースするテストは結合テストとして別途実施する。

## 推奨されるモジュール構造

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

## 推奨テストケース

| テスト名 | 入力 | 期待結果 |
|---------|------|----------|
| test_analyze_simple_function | `fn main() {}` | Scope に main 関数が存在 |
| test_analyze_variable_decl | `var x = 1` | 変数 x が定義される |
| test_analyze_function_args | `fn f(a, b) {}` | 引数 a, b が定義される |
| test_analyze_nested_scope | `{ var x = 1 }` | スコープ内の変数解決 |
| test_analyze_recursive_call | `fn f() { f() }` | 再帰呼び出しの解決 |
| test_convert_expression | 数値式 | ExecExpression への変換 |
| test_convert_statement | 代入文 | ExecStatement への変換 |
| test_error_undefined_var | `x = 1` | エラー: 未定義変数 |
| test_error_duplicate_func | `fn f() {} fn f() {}` | エラー: 重複定義 |

## 優先度

**中** - interpreter と同等、サブモジュール分割により大幅改善可能

## 参考

- 元の分析: [unit-test-analysis.md](../done-task/unit-test-analysis.md)
