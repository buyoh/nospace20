use super::*;

// テストヘルパー: トークン生成関数
fn token_number(value: i64) -> PrettyToken {
    (Token::Number(value), TokenInfo { code_pointer: 0 })
}

fn token_ident(name: &str) -> PrettyToken {
    (
        Token::Identifier(name.to_string()),
        TokenInfo { code_pointer: 0 },
    )
}

fn token_op_plus() -> PrettyToken {
    (Token::Plus, TokenInfo { code_pointer: 0 })
}

fn token_op_minus() -> PrettyToken {
    (Token::Minus, TokenInfo { code_pointer: 0 })
}

fn token_op_asterisk() -> PrettyToken {
    (Token::Asterisk, TokenInfo { code_pointer: 0 })
}

fn token_op_slash() -> PrettyToken {
    (Token::Slash, TokenInfo { code_pointer: 0 })
}

fn token_op_percent() -> PrettyToken {
    (Token::Percent, TokenInfo { code_pointer: 0 })
}

fn token_op_double_equal() -> PrettyToken {
    (Token::DoubleEqual, TokenInfo { code_pointer: 0 })
}

fn token_op_not_equal() -> PrettyToken {
    (Token::NotEqual, TokenInfo { code_pointer: 0 })
}

fn token_op_less() -> PrettyToken {
    (Token::Less, TokenInfo { code_pointer: 0 })
}

fn token_op_less_equal() -> PrettyToken {
    (Token::LessEqual, TokenInfo { code_pointer: 0 })
}

fn token_op_greater() -> PrettyToken {
    (Token::Greater, TokenInfo { code_pointer: 0 })
}

fn token_op_greater_equal() -> PrettyToken {
    (Token::GreaterEqual, TokenInfo { code_pointer: 0 })
}

fn token_op_logical_and() -> PrettyToken {
    (Token::DoubleAmpersand, TokenInfo { code_pointer: 0 })
}

fn token_op_logical_or() -> PrettyToken {
    (Token::DoublePipe, TokenInfo { code_pointer: 0 })
}

fn token_op_single_equal() -> PrettyToken {
    (Token::SingleEqual, TokenInfo { code_pointer: 0 })
}

fn token_op_exclamation() -> PrettyToken {
    (Token::Exclamation, TokenInfo { code_pointer: 0 })
}

fn token_paren_l() -> PrettyToken {
    (Token::ParenthesisL, TokenInfo { code_pointer: 0 })
}

fn token_paren_r() -> PrettyToken {
    (Token::ParenthesisR, TokenInfo { code_pointer: 0 })
}

fn token_comma() -> PrettyToken {
    (Token::Comma, TokenInfo { code_pointer: 0 })
}

fn token_bracket_l() -> PrettyToken {
    (Token::BracketL, TokenInfo { code_pointer: 0 })
}

fn token_bracket_r() -> PrettyToken {
    (Token::BracketR, TokenInfo { code_pointer: 0 })
}

// ヘルパー: パース実行
fn parse_expr(tokens: Vec<PrettyToken>) -> (Box<LocatedExpression>, Vec<CodeParseError>) {
    parse_to_expression_tree_root(&mut tokens.iter().peekable())
}

#[test]
fn test_parse_literal_number() {
    let tokens = vec![token_number(42)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Factor(val) => assert_eq!(val, 42),
        _ => panic!("Expected Expression::Factor"),
    }
}

#[test]
fn test_parse_variable() {
    let tokens = vec![token_ident("foo")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Variable(name) => assert_eq!(name, "foo"),
        _ => panic!("Expected Expression::Variable"),
    }
}

#[test]
fn test_parse_add() {
    let tokens = vec![token_number(1), token_op_plus(), token_number(2)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Plus, left, right) => match (left.expression, right.expression) {
            (Expression::Factor(1), Expression::Factor(2)) => (),
            _ => panic!("Expected Factor(1) + Factor(2)"),
        },
        _ => panic!("Expected Expression::Operation2(Plus)"),
    }
}

#[test]
fn test_parse_subtract() {
    let tokens = vec![token_number(5), token_op_minus(), token_number(3)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Minus, left, right) => match (left.expression, right.expression) {
            (Expression::Factor(5), Expression::Factor(3)) => (),
            _ => panic!("Expected Factor(5) - Factor(3)"),
        },
        _ => panic!("Expected Expression::Operation2(Minus)"),
    }
}

#[test]
fn test_parse_multiply() {
    let tokens = vec![token_number(3), token_op_asterisk(), token_number(4)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Multiply, left, right) => match (left.expression, right.expression) {
            (Expression::Factor(3), Expression::Factor(4)) => (),
            _ => panic!("Expected Factor(3) * Factor(4)"),
        },
        _ => panic!("Expected Expression::Operation2(Multiply)"),
    }
}

#[test]
fn test_parse_divide() {
    let tokens = vec![token_number(10), token_op_slash(), token_number(2)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Divide, left, right) => match (left.expression, right.expression) {
            (Expression::Factor(10), Expression::Factor(2)) => (),
            _ => panic!("Expected Factor(10) / Factor(2)"),
        },
        _ => panic!("Expected Expression::Operation2(Divide)"),
    }
}

#[test]
fn test_parse_modulo() {
    let tokens = vec![token_number(10), token_op_percent(), token_number(3)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Modulo, left, right) => match (left.expression, right.expression) {
            (Expression::Factor(10), Expression::Factor(3)) => (),
            _ => panic!("Expected Factor(10) % Factor(3)"),
        },
        _ => panic!("Expected Expression::Operation2(Modulo)"),
    }
}

#[test]
fn test_parse_precedence_mul_before_add() {
    // 1 + 2 * 3 => 1 + (2 * 3)
    let tokens = vec![
        token_number(1),
        token_op_plus(),
        token_number(2),
        token_op_asterisk(),
        token_number(3),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Plus, left, right) => match (left.expression, right.expression) {
            (Expression::Factor(1), Expression::Operation2(Operator2::Multiply, l2, r2)) => {
                match (l2.expression, r2.expression) {
                    (Expression::Factor(2), Expression::Factor(3)) => (),
                    _ => panic!("Expected Factor(2) * Factor(3)"),
                }
            }
            _ => panic!("Expected 1 + (2 * 3)"),
        },
        _ => panic!("Expected Expression::Operation2(Plus)"),
    }
}

#[test]
fn test_parse_parenthesis() {
    // (1 + 2) * 3 => (1 + 2) * 3
    let tokens = vec![
        token_paren_l(),
        token_number(1),
        token_op_plus(),
        token_number(2),
        token_paren_r(),
        token_op_asterisk(),
        token_number(3),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Multiply, left, right) => match (left.expression, right.expression) {
            (Expression::Operation2(Operator2::Plus, l2, r2), Expression::Factor(3)) => {
                match (l2.expression, r2.expression) {
                    (Expression::Factor(1), Expression::Factor(2)) => (),
                    _ => panic!("Expected Factor(1) + Factor(2)"),
                }
            }
            _ => panic!("Expected (1 + 2) * 3"),
        },
        _ => panic!("Expected Expression::Operation2(Multiply)"),
    }
}

#[test]
fn test_parse_function_call_no_args() {
    // foo()
    let tokens = vec![token_ident("foo"), token_paren_l(), token_paren_r()];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Function(name, args) => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 0);
        }
        _ => panic!("Expected Expression::Function"),
    }
}

#[test]
fn test_parse_function_call_one_arg() {
    // foo(42)
    let tokens = vec![
        token_ident("foo"),
        token_paren_l(),
        token_number(42),
        token_paren_r(),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Function(name, args) => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 1);
            match args[0].expression {
                Expression::Factor(42) => (),
                _ => panic!("Expected Factor(42)"),
            }
        }
        _ => panic!("Expected Expression::Function"),
    }
}

#[test]
fn test_parse_function_call_multi_args() {
    // foo(1, 2)
    let tokens = vec![
        token_ident("foo"),
        token_paren_l(),
        token_number(1),
        token_comma(),
        token_number(2),
        token_paren_r(),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Function(name, args) => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 2);
            match (args[0].expression.clone(), args[1].expression.clone()) {
                (Expression::Factor(1), Expression::Factor(2)) => (),
                _ => panic!("Expected Factor(1), Factor(2)"),
            }
        }
        _ => panic!("Expected Expression::Function"),
    }
}

#[test]
fn test_parse_unary_minus() {
    // -1
    let tokens = vec![token_op_minus(), token_number(1)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation1(Operator1::Negative, inner) => match inner.expression {
            Expression::Factor(1) => (),
            _ => panic!("Expected Factor(1)"),
        },
        _ => panic!("Expected Expression::Operation1(Negative)"),
    }
}

#[test]
fn test_parse_unary_logical_not() {
    // !true (represented as !1)
    let tokens = vec![token_op_exclamation(), token_number(1)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation1(Operator1::LogicalNot, inner) => match inner.expression {
            Expression::Factor(1) => (),
            _ => panic!("Expected Factor(1)"),
        },
        _ => panic!("Expected Expression::Operation1(LogicalNot)"),
    }
}

#[test]
fn test_parse_comparison_equal() {
    // a == b
    let tokens = vec![token_ident("a"), token_op_double_equal(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Equal, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) == Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(Equal)"),
    }
}

#[test]
fn test_parse_comparison_not_equal() {
    // a != b
    let tokens = vec![token_ident("a"), token_op_not_equal(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::NotEqual, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) != Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(NotEqual)"),
    }
}

#[test]
fn test_parse_comparison_less() {
    // a < b
    let tokens = vec![token_ident("a"), token_op_less(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Less, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) < Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(Less)"),
    }
}

#[test]
fn test_parse_comparison_less_equal() {
    // a <= b
    let tokens = vec![token_ident("a"), token_op_less_equal(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::LessEqual, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) <= Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(LessEqual)"),
    }
}

#[test]
fn test_parse_comparison_greater() {
    // a > b
    let tokens = vec![token_ident("a"), token_op_greater(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Greater, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) > Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(Greater)"),
    }
}

#[test]
fn test_parse_comparison_greater_equal() {
    // a >= b
    let tokens = vec![token_ident("a"), token_op_greater_equal(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::GreaterEqual, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) >= Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(GreaterEqual)"),
    }
}

#[test]
fn test_parse_logical_and() {
    // a && b
    let tokens = vec![token_ident("a"), token_op_logical_and(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::LogicalAnd, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) && Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(LogicalAnd)"),
    }
}

#[test]
fn test_parse_logical_or() {
    // a || b
    let tokens = vec![token_ident("a"), token_op_logical_or(), token_ident("b")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::LogicalOr, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Variable(b)) => {
                assert_eq!(a, "a");
                assert_eq!(b, "b");
            }
            _ => panic!("Expected Variable(a) || Variable(b)"),
        },
        _ => panic!("Expected Expression::Operation2(LogicalOr)"),
    }
}

#[test]
fn test_parse_assignment() {
    // a = 10
    let tokens = vec![token_ident("a"), token_op_single_equal(), token_number(10)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Assign, left, right) => match (left.expression, right.expression) {
            (Expression::Variable(a), Expression::Factor(10)) => {
                assert_eq!(a, "a");
            }
            _ => panic!("Expected Variable(a) = Factor(10)"),
        },
        _ => panic!("Expected Expression::Operation2(Assign)"),
    }
}

#[test]
fn test_parse_error_unclosed_paren() {
    // (1 + 2  (missing close paren)
    let tokens = vec![
        token_paren_l(),
        token_number(1),
        token_op_plus(),
        token_number(2),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(!errs.is_empty(), "Expected errors for unclosed paren");
    // エラーが発生することを確認
    match expr.expression {
        Expression::Operation2(Operator2::Plus, _, _) => (), // パースは進むがエラーも記録される
        _ => {}
    }
}

#[test]
fn test_parse_double_unary_minus() {
    // --5 => -(-5)
    let tokens = vec![token_op_minus(), token_op_minus(), token_number(5)];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation1(Operator1::Negative, inner1) => match inner1.expression {
            Expression::Operation1(Operator1::Negative, inner2) => match inner2.expression {
                Expression::Factor(5) => (),
                _ => panic!("Expected Factor(5)"),
            },
            _ => panic!("Expected Operation1(Negative)"),
        },
        _ => panic!("Expected Expression::Operation1(Negative)"),
    }
}

#[test]
fn test_parse_complex_precedence() {
    // 1 + 2 * 3 < 10 && 5 == 5
    let tokens = vec![
        token_number(1),
        token_op_plus(),
        token_number(2),
        token_op_asterisk(),
        token_number(3),
        token_op_less(),
        token_number(10),
        token_op_logical_and(),
        token_number(5),
        token_op_double_equal(),
        token_number(5),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    // 複雑な優先順位が正しく処理されることを確認
    match expr.expression {
        Expression::Operation2(Operator2::LogicalAnd, _, _) => (),
        _ => panic!("Expected top-level LogicalAnd"),
    }
}
// テストヘルパー: 参照・デリファレンス演算子
fn token_op_ampersand() -> PrettyToken {
    (Token::Ampersand, TokenInfo { code_pointer: 0 })
}

// 参照演算子のテスト
#[test]
fn test_parse_reference_operator() {
    let tokens = vec![token_op_ampersand(), token_ident("x")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation1(Operator1::Ref, inner) => match inner.expression {
            Expression::Variable(_) => (),
            _ => panic!("Expected inner expression to be Variable"),
        },
        _ => panic!("Expected Expression::Operation1 with Operator1::Ref"),
    }
}

// デリファレンス演算子のテスト
#[test]
fn test_parse_dereference_operator() {
    let tokens = vec![token_op_asterisk(), token_ident("p")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation1(Operator1::Deref, inner) => match inner.expression {
            Expression::Variable(_) => (),
            _ => panic!("Expected inner expression to be Variable"),
        },
        _ => panic!("Expected Expression::Operation1 with Operator1::Deref"),
    }
}

// ダブルデリファレンスのテスト: **p
#[test]
fn test_parse_double_dereference() {
    let tokens = vec![token_op_asterisk(), token_op_asterisk(), token_ident("p")];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation1(Operator1::Deref, inner) => match inner.expression {
            Expression::Operation1(Operator1::Deref, inner2) => match inner2.expression {
                Expression::Variable(_) => (),
                _ => panic!("Expected innermost expression to be Variable"),
            },
            _ => panic!("Expected inner expression to be Deref"),
        },
        _ => panic!("Expected Expression::Operation1 with Operator1::Deref"),
    }
}

// * が単項と二項で正しく区別されることを確認: a * *p
#[test]
fn test_parse_multiply_and_dereference() {
    let tokens = vec![
        token_ident("a"),
        token_op_asterisk(),
        token_op_asterisk(),
        token_ident("p"),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    match expr.expression {
        Expression::Operation2(Operator2::Multiply, left, right) => {
            match left.expression {
                Expression::Variable(_) => (),
                _ => panic!("Expected left to be Variable"),
            }
            match right.expression {
                Expression::Operation1(Operator1::Deref, _) => (),
                _ => panic!("Expected right to be Deref"),
            }
        }
        _ => panic!("Expected Expression::Operation2 with Operator2::Multiply"),
    }
}

// 配列アクセス: arr[0]
#[test]
fn test_parse_array_access_literal_index() {
    let tokens = vec![
        token_ident("arr"),
        token_bracket_l(),
        token_number(0),
        token_bracket_r(),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors, got: {:?}", errs);
    match expr.expression {
        Expression::ArrayAccess(name, index) => {
            assert_eq!(name, "arr");
            match index.expression {
                Expression::Factor(0) => (),
                _ => panic!("Expected Factor(0) as index"),
            }
        }
        _ => panic!("Expected Expression::ArrayAccess, got: {:?}", expr),
    }
}

// 配列アクセス: arr[i+1]
#[test]
fn test_parse_array_access_expr_index() {
    let tokens = vec![
        token_ident("arr"),
        token_bracket_l(),
        token_ident("i"),
        token_op_plus(),
        token_number(1),
        token_bracket_r(),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors, got: {:?}", errs);
    match expr.expression {
        Expression::ArrayAccess(name, index) => {
            assert_eq!(name, "arr");
            match index.expression {
                Expression::Operation2(Operator2::Plus, left, right) => match (left.expression, right.expression) {
                    (Expression::Variable(v), Expression::Factor(1)) => {
                        assert_eq!(v, "i");
                    }
                    _ => panic!("Expected Variable(i) + Factor(1)"),
                },
                _ => panic!("Expected Operation2(Plus) as index"),
            }
        }
        _ => panic!("Expected Expression::ArrayAccess"),
    }
}

// 配列アクセスを含む代入: arr[0] = 5
#[test]
fn test_parse_array_assign() {
    let tokens = vec![
        token_ident("arr"),
        token_bracket_l(),
        token_number(0),
        token_bracket_r(),
        token_op_single_equal(),
        token_number(5),
    ];
    let (expr, errs) = parse_expr(tokens);
    assert!(errs.is_empty(), "Expected no errors, got: {:?}", errs);
    match expr.expression {
        Expression::Operation2(Operator2::Assign, left, right) => {
            match left.expression {
                Expression::ArrayAccess(name, index) => {
                    assert_eq!(name, "arr");
                    match index.expression {
                        Expression::Factor(0) => (),
                        _ => panic!("Expected Factor(0)"),
                    }
                }
                _ => panic!("Expected ArrayAccess on left side"),
            }
            match right.expression {
                Expression::Factor(5) => (),
                _ => panic!("Expected Factor(5) on right side"),
            }
        }
        _ => panic!("Expected Expression::Operation2(Assign)"),
    }
}
