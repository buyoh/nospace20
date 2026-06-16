# tree_parser 後置添字演算子ユニットテスト失敗調査

## 概要

`fix-postfix-subscript-semantics` タスクの実装 (tree_parser の脱糖ルール変更) により、
`src/tree_parser/expression/test.rs` の3つのユニットテストが失敗するようになった。

## 失敗テスト

| テスト名 | ファイル | 行 |
|---|---|---|
| `test_parse_postfix_subscript_deref_paren` | `src/tree_parser/expression/test.rs` | L764 |
| `test_parse_postfix_subscript_deref_paren_index_1` | `src/tree_parser/expression/test.rs` | L800 |
| `test_parse_postfix_subscript_expr_paren` | `src/tree_parser/expression/test.rs` | L836 |

## 原因

これらのテストは **旧仕様** の脱糖ルール `(expr)[i] → *(expr + i)` に基づいて
期待 AST 構造を記述していた。

新仕様では `(expr)[i] → *(&(expr) + i)` に変更されたため、生成される AST が変わった:

| | 旧 AST | 新 AST |
|---|---|---|
| `(*p)[0]` | `Deref(Plus(Deref(Variable("p")), Factor(0)))` | `Deref(Plus(Ref(Deref(Variable("p"))), Factor(0)))` |
| `(*p)[1]` | `Deref(Plus(Deref(Variable("p")), Factor(1)))` | `Deref(Plus(Ref(Deref(Variable("p"))), Factor(1)))` |
| `(x+y)[2]` | `Deref(Plus(Plus(Variable("x"), Variable("y")), Factor(2)))` | `Deref(Plus(Ref(Plus(Variable("x"), Variable("y"))), Factor(2)))` |

## 対応方法

これらのテストを新仕様に合わせて更新する必要がある:

### test_parse_postfix_subscript_deref_paren / test_parse_postfix_subscript_deref_paren_index_1

期待する AST を `Deref(Plus(Ref(Deref(Variable("p"))), Factor(n)))` に修正する:

```rust
match expr.expression {
    Expression::Operation1(Operator1::Deref, inner) => match inner.expression {
        Expression::Operation2(Operator2::Plus, left, right) => {
            match left.expression {
                Expression::Operation1(Operator1::Ref, ref_inner) => {
                    match ref_inner.expression {
                        Expression::Operation1(Operator1::Deref, p) => match p.expression {
                            Expression::Variable(name) => assert_eq!(name, "p"),
                            _ => panic!("Expected Variable(p)"),
                        },
                        _ => panic!("Expected Deref(Variable(p)) inside Ref"),
                    }
                }
                _ => panic!("Expected Ref(...) on left"),
            }
            // ...
        }
    }
}
```

### test_parse_postfix_subscript_expr_paren

期待する AST を `Deref(Plus(Ref(Plus(Variable("x"), Variable("y"))), Factor(2)))` に修正する:

```rust
match left.expression {
    Expression::Operation1(Operator1::Ref, ref_inner) => {
        match ref_inner.expression {
            Expression::Operation2(Operator2::Plus, ll, lr) => {
                // x, y の確認
            }
            _ => panic!("Expected Plus(x, y) inside Ref"),
        }
    }
    _ => panic!("Expected Ref(Plus(x, y)) on left"),
}
```

## 優先度

低。largeテスト (code_test) は全て通過しており、仕様の動作は正しく実装されている。
ユニットテストの期待値更新のみ必要。
