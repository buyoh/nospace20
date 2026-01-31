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

- [ ] **T1-1**: トークン列を手動構築するためのビルダー追加
  ```rust
  #[cfg(test)]
  mod test_helpers {
      use super::*;
      
      pub fn token_number(value: i64) -> PrettyToken {
          PrettyToken { token: Token::Number(value), .. }
      }
      pub fn token_ident(name: &str) -> PrettyToken {
          PrettyToken { token: Token::Identifier(name.to_string()), .. }
      }
      pub fn token_op(op: &str) -> PrettyToken {
          PrettyToken { token: Token::Operator(op.to_string()), .. }
      }
      // 他のトークン種別も同様に追加
  }
  ```
- [ ] **T1-2**: 外部ファイル（JSON/YAML）によるテストケース定義の検討
  - テストケースが大きくなる場合、可読性のため外部ファイル化を検討
  - `resources/unit-tests/tree_parser/` にテストデータを配置

### Phase 2: ユニットテスト追加

- [ ] **T2-1**: tree_parser のユニットテスト追加（10件程度）
  - 基本的な式のパース（トークン列を手動構築）
  - 演算子優先順位
  - 関数呼び出し
  - エラーケース

**注意**: ユニットテストでは `token_parser` に依存せず、トークン列を直接構築すること。
`token_parser::parse_to_tokens()` を使用するテストは結合テストとして別途実施する。

## 推奨される設計変更

### Option A: pub(crate) での公開

```rust
// expression.rs
pub(crate) fn parse_to_expression_tree_root(...) -> ...
```

### Option B: テスト専用モジュール（推奨）

```rust
#[cfg(test)]
mod test {
    use super::*;
    use super::test_helpers::*;
    
    #[test]
    fn test_parse_simple_expression() {
        // トークン列を手動構築（token_parser に依存しない）
        let tokens = vec![
            token_number(1),
            token_op("+"),
            token_number(2),
        ];
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
