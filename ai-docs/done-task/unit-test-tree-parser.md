# tree_parser ユニットテスト追加

## 概要

tree_parser モジュールにユニットテストを追加するためのタスク。

## ステータス

**完了** - 2026-02-01

## 実施内容

### Phase 1: テストヘルパー整備

- [x] **T1-1**: トークン列を手動構築するためのビルダー追加
  - `src/tree_parser/expression.rs` のテストモジュールに実装
  - `token_number()`, `token_ident()`, `token_op_*()` 等のヘルパー関数を追加
  - `parse_expr()` ヘルパーでパース実行を簡易化

### Phase 2: ユニットテスト追加

- [x] **T2-1**: expression パーサーのユニットテスト追加（26件）
  - リテラル: `test_parse_literal_number`, `test_parse_variable`
  - 算術演算子: `test_parse_add`, `test_parse_subtract`, `test_parse_multiply`, `test_parse_divide`, `test_parse_modulo`
  - 演算子優先順位: `test_parse_precedence_mul_before_add`, `test_parse_parenthesis`, `test_parse_complex_precedence`
  - 関数呼び出し: `test_parse_function_call_no_args`, `test_parse_function_call_one_arg`, `test_parse_function_call_multi_args`
  - 単項演算子: `test_parse_unary_minus`, `test_parse_unary_logical_not`, `test_parse_double_unary_minus`
  - 比較演算子: `test_parse_comparison_equal`, `test_parse_comparison_not_equal`, `test_parse_comparison_less`, `test_parse_comparison_less_equal`, `test_parse_comparison_greater`, `test_parse_comparison_greater_equal`
  - 論理演算子: `test_parse_logical_and`, `test_parse_logical_or`
  - 代入: `test_parse_assignment`
  - エラーケース: `test_parse_error_unclosed_paren`

- [x] **T2-2**: statement パーサーのユニットテスト追加（11件）
  - 変数宣言: `test_parse_let_statement`
  - 制御フロー: `test_parse_break_statement`, `test_parse_continue_statement`, `test_parse_return_statement`
  - 式文: `test_parse_expression_statement`
  - 関数宣言: `test_parse_func_no_args`, `test_parse_func_one_arg`, `test_parse_func_multi_args`, `test_parse_func_with_body`
  - 複数文: `test_parse_multiple_statements`, `test_parse_empty_statements`

## テスト結果

```
running 37 tests (tree_parser module)
test result: ok. 37 passed; 0 failed; 0 ignored
```

全体: 61 passed (tree_parser: 37 + その他モジュール: 24)

## 実装の詳細

### テストヘルパー関数

トークン列を手動で構築するため、以下のヘルパー関数を実装:

```rust
// 数値トークン
fn token_number(value: i64) -> PrettyToken

// 識別子トークン
fn token_ident(name: &str) -> PrettyToken

// 演算子トークン
fn token_op_plus() -> PrettyToken
fn token_op_minus() -> PrettyToken
// ... その他の演算子

// キーワードトークン
fn token_keyword_let() -> PrettyToken
fn token_keyword_func() -> PrettyToken
// ... その他のキーワード

// 区切り記号
fn token_paren_l/r() -> PrettyToken
fn token_brace_l/r() -> PrettyToken
// ... その他
```

### テスト設計

- **ユニットテストの方針**: `token_parser` に依存せず、トークン列を直接構築
- **カバレッジ**: 主要な言語機能を網羅
  - 全算術演算子 (+, -, *, /, %)
  - 全比較演算子 (==, !=, <, <=, >, >=)
  - 論理演算子 (&&, ||, !)
  - 演算子優先順位
  - 括弧による優先順位変更
  - 関数呼び出し（引数0個、1個、複数）
  - 各種文（let, func, return, break, continue）
  - エラーハンドリング

## 利点

1. **高速なフィードバック**: トークンパーサーに依存しないため、テストが高速
2. **詳細なテスト**: 個別の構文要素を独立してテスト可能
3. **リグレッション防止**: パーサーの変更時に既存機能が壊れていないか確認
4. **ドキュメント効果**: テストコードが文法の使用例として機能

## 参考

- 元の分析: [unit-test-analysis.md](../done-task/unit-test-analysis.md)
- 実装ファイル: 
  - [src/tree_parser/expression.rs](../../src/tree_parser/expression.rs)
  - [src/tree_parser/statement.rs](../../src/tree_parser/statement.rs)
