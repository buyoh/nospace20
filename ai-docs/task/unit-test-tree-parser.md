# tree_parser ユニットテスト追加

## 概要

tree_parser モジュールにユニットテストを追加するためのタスク。
現状ユニットテストがなく、追加には設計変更が必要。

## 背景

[unit-test-analysis.md](../done-task/unit-test-analysis.md) の分析結果より分離。

## 現状の課題

1. **内部構造へのアクセス不可**: `ExpressionBuilder::parse()` や `StatementBuilder::parse()` は private
2. **テスト用ヘルパーなし**: トークン列を簡単に生成するヘルパーがない
3. **公開インターフェースは `parse_to_tree()` のみ**

## 改善タスク

### Phase 1: テストヘルパー整備

- [ ] **T1-1**: tree_parser 用ヘルパー関数追加
  ```rust
  #[cfg(test)]
  pub(crate) fn parse_expression_from_str(code: &str) -> Box<Expression>
  ```
- [ ] **T1-2**: tokens_from_str ヘルパー追加
  ```rust
  #[cfg(test)]
  fn tokens_from_str(s: &str) -> Vec<PrettyToken> {
      crate::token_parser::parse_to_tokens(s).unwrap()
  }
  ```

### Phase 2: ユニットテスト追加

- [ ] **T2-1**: tree_parser のユニットテスト追加（10件程度）
  - 基本的な式のパース
  - 演算子優先順位
  - 関数呼び出し
  - エラーケース

## 推奨される設計変更

### Option A: pub(crate) での公開

```rust
// expression.rs
pub(crate) fn parse_to_expression_tree_root(...) -> ...
```

### Option B: テスト専用モジュール

```rust
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

## 推奨テストケース

| テスト名 | 入力 | 期待結果 |
|---------|------|----------|
| test_parse_literal | `42` | Expression::Number(42) |
| test_parse_add | `1 + 2` | Expression::BinaryOperator(+, 1, 2) |
| test_parse_precedence | `1 + 2 * 3` | 乗算が優先 |
| test_parse_paren | `(1 + 2) * 3` | 括弧内が優先 |
| test_parse_function_call | `foo(1, 2)` | Expression::FunctionCall |
| test_parse_unary_minus | `-1` | Expression::UnaryOperator |
| test_parse_comparison | `a < b` | Expression::BinaryOperator(<) |
| test_parse_logical_and | `a && b` | Expression::BinaryOperator(&&) |
| test_parse_logical_or | `a || b` | Expression::BinaryOperator(\|\|) |
| test_error_unclosed_paren | `(1 + 2` | エラー |

## 優先度

**高** - パーサーはコンパイラの基盤であり、早期にテストを整備すべき

## 参考

- 元の分析: [unit-test-analysis.md](../done-task/unit-test-analysis.md)
