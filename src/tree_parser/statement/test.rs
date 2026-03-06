use super::*;
use crate::tree_parser::expression::{Expression, Operator2};

// テストヘルパー: トークン生成関数
fn token_keyword_let() -> PrettyToken {
    (Token::Keyword(Keyword::Let), TokenInfo { code_pointer: 0 })
}

fn token_keyword_func() -> PrettyToken {
    (Token::Keyword(Keyword::Func), TokenInfo { code_pointer: 0 })
}

fn token_keyword_return() -> PrettyToken {
    (
        Token::Keyword(Keyword::Return),
        TokenInfo { code_pointer: 0 },
    )
}

fn token_keyword_break() -> PrettyToken {
    (
        Token::Keyword(Keyword::Break),
        TokenInfo { code_pointer: 0 },
    )
}

fn token_keyword_continue() -> PrettyToken {
    (
        Token::Keyword(Keyword::Continue),
        TokenInfo { code_pointer: 0 },
    )
}

fn token_ident(name: &str) -> PrettyToken {
    (
        Token::Identifier(name.to_string()),
        TokenInfo { code_pointer: 0 },
    )
}

fn token_number(value: i64) -> PrettyToken {
    (Token::Number(value), TokenInfo { code_pointer: 0 })
}

#[allow(dead_code)]
fn token_colon() -> PrettyToken {
    (Token::Colon, TokenInfo { code_pointer: 0 })
}

fn token_semicolon() -> PrettyToken {
    (Token::Semicolon, TokenInfo { code_pointer: 0 })
}

fn token_paren_l() -> PrettyToken {
    (Token::ParenthesisL, TokenInfo { code_pointer: 0 })
}

fn token_paren_r() -> PrettyToken {
    (Token::ParenthesisR, TokenInfo { code_pointer: 0 })
}

fn token_brace_l() -> PrettyToken {
    (Token::BraceL, TokenInfo { code_pointer: 0 })
}

fn token_brace_r() -> PrettyToken {
    (Token::BraceR, TokenInfo { code_pointer: 0 })
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

fn token_op_single_equal() -> PrettyToken {
    (Token::SingleEqual, TokenInfo { code_pointer: 0 })
}

// ヘルパー: パース実行
fn parse_stmts(tokens: Vec<PrettyToken>) -> (Vec<LocatedStatement>, Vec<CodeParseError>) {
    parse_to_statements(&mut tokens.iter().peekable())
}

#[test]
fn test_parse_let_statement() {
    // let: x;
    let tokens = vec![token_keyword_let(), token_ident("x"), token_semicolon()];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, expr, is_static, _, array_size) => {
            assert_eq!(name, "x");
            assert_eq!(*is_static, false); // non-static
            assert_eq!(*array_size, None); // not an array
            match expr.expression {
                Expression::Factor(0) => (), // デフォルト値は0
                _ => panic!("Expected Factor(0)"),
            }
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
}

#[test]
fn test_parse_break_statement() {
    // break;
    let tokens = vec![token_keyword_break(), token_semicolon()];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::Break => (),
        _ => panic!("Expected Statement::Break"),
    }
}

#[test]
fn test_parse_continue_statement() {
    // continue;
    let tokens = vec![token_keyword_continue(), token_semicolon()];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::Continue => (),
        _ => panic!("Expected Statement::Continue"),
    }
}

#[test]
fn test_parse_return_statement() {
    // return: 42;
    let tokens = vec![token_keyword_return(), token_number(42), token_semicolon()];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::Return(Some(expr)) => match expr.expression {
            Expression::Factor(42) => (),
            _ => panic!("Expected Factor(42)"),
        },
        _ => panic!("Expected Statement::Return(Some(...))"),
    }
}

#[test]
fn test_parse_void_return_with_colon() {
    // return:;
    let tokens = vec![token_keyword_return(), token_semicolon()];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::Return(None) => (),
        _ => panic!("Expected Statement::Return(None)"),
    }
}

#[test]
fn test_parse_void_return_without_colon() {
    // return;
    let tokens = vec![token_keyword_return(), token_semicolon()];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::Return(None) => (),
        _ => panic!("Expected Statement::Return(None)"),
    }
}

#[test]
fn test_parse_expression_statement() {
    // x = 10;
    let tokens = vec![
        token_ident("x"),
        token_op_single_equal(),
        token_number(10),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::Expression(expr) => match expr.expression {
            Expression::Operation2(Operator2::Assign, _, _) => (),
            _ => panic!("Expected Operation2(Assign)"),
        },
        _ => panic!("Expected Statement::Expression"),
    }
}

#[test]
fn test_parse_func_no_args() {
    // func: foo() {}
    let tokens = vec![
        token_keyword_func(),
        token_ident("foo"),
        token_paren_l(),
        token_paren_r(),
        token_brace_l(),
        token_brace_r(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::FunctionDeclaration(name, args, body) => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 0);
            assert_eq!(body.len(), 0);
        }
        _ => panic!("Expected Statement::FunctionDeclaration"),
    }
}

#[test]
fn test_parse_func_one_arg() {
    // func: bar(x) {}
    let tokens = vec![
        token_keyword_func(),
        token_ident("bar"),
        token_paren_l(),
        token_ident("x"),
        token_paren_r(),
        token_brace_l(),
        token_brace_r(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::FunctionDeclaration(name, args, body) => {
            assert_eq!(name, "bar");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], "x");
            assert_eq!(body.len(), 0);
        }
        _ => panic!("Expected Statement::FunctionDeclaration"),
    }
}

#[test]
fn test_parse_func_multi_args() {
    // func: baz(x, y) {}
    let tokens = vec![
        token_keyword_func(),
        token_ident("baz"),
        token_paren_l(),
        token_ident("x"),
        token_comma(),
        token_ident("y"),
        token_paren_r(),
        token_brace_l(),
        token_brace_r(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::FunctionDeclaration(name, args, body) => {
            assert_eq!(name, "baz");
            assert_eq!(args.len(), 2);
            assert_eq!(args[0], "x");
            assert_eq!(args[1], "y");
            assert_eq!(body.len(), 0);
        }
        _ => panic!("Expected Statement::FunctionDeclaration"),
    }
}

#[test]
fn test_parse_func_with_body() {
    // func: foo() { return: 42; }
    let tokens = vec![
        token_keyword_func(),
        token_ident("foo"),
        token_paren_l(),
        token_paren_r(),
        token_brace_l(),
        token_keyword_return(),
        token_number(42),
        token_semicolon(),
        token_brace_r(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::FunctionDeclaration(name, args, body) => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 0);
            assert_eq!(body.len(), 1);
            match &body[0].statement {
                Statement::Return(_) => (),
                _ => panic!("Expected Statement::Return in body"),
            }
        }
        _ => panic!("Expected Statement::FunctionDeclaration"),
    }
}

#[test]
fn test_parse_multiple_statements() {
    // let: x;
    // let: y;
    let tokens = vec![
        token_keyword_let(),
        token_ident("x"),
        token_semicolon(),
        token_keyword_let(),
        token_ident("y"),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 2);
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, _, _, _, _) => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
    match &stmts[1].statement {
        Statement::VariableDeclaration(name, _, _, _, _) => {
            assert_eq!(name, "y");
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
}

// 配列宣言: let: arr[4];
#[test]
fn test_parse_array_declaration() {
    let tokens = vec![
        token_keyword_let(),
        token_ident("arr"),
        token_bracket_l(),
        token_number(4),
        token_bracket_r(),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors, got: {:?}", errs);
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, expr, is_static, _, array_size) => {
            assert_eq!(name, "arr");
            assert_eq!(*is_static, false);
            assert_eq!(*array_size, Some(4));
            match expr.expression {
                Expression::Factor(0) => (), // デフォルト初期化
                _ => panic!("Expected Factor(0)"),
            }
        }
        _ => panic!(
            "Expected Statement::VariableDeclaration, got: {:?}",
            stmts[0].statement
        ),
    }
}

// 配列宣言（初期化あり）: let: arr[3]([10, 20, 30]);
#[test]
fn test_parse_array_declaration_with_init() {
    let tokens = vec![
        token_keyword_let(),
        token_ident("arr"),
        token_bracket_l(),
        token_number(3),
        token_bracket_r(),
        token_paren_l(),
        token_bracket_l(),
        token_number(10),
        token_comma(),
        token_number(20),
        token_comma(),
        token_number(30),
        token_bracket_r(),
        token_paren_r(),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors, got: {:?}", errs);
    // 宣言 + 初期化式3つ = 4文
    assert_eq!(
        stmts.len(),
        4,
        "Expected 4 statements (1 declaration + 3 assignments)"
    );

    // 1つ目: 配列宣言
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, _, is_static, _, array_size) => {
            assert_eq!(name, "arr");
            assert_eq!(*is_static, false);
            assert_eq!(*array_size, Some(3));
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }

    // 2-4つ目: 各要素への代入 arr[0]=10, arr[1]=20, arr[2]=30
    for (i, expected_val) in [10, 20, 30].iter().enumerate() {
        match &stmts[i + 1].statement {
            Statement::Expression(expr) => match &expr.expression {
                Expression::Operation2(Operator2::Assign, left, right) => {
                    match &left.expression {
                        Expression::ArrayAccess(name, index) => {
                            assert_eq!(name, "arr");
                            match &index.expression {
                                Expression::Factor(idx) => {
                                    assert_eq!(*idx, i as i64);
                                }
                                _ => panic!("Expected Factor as index"),
                            }
                        }
                        _ => panic!("Expected ArrayAccess on left side"),
                    }
                    match &right.expression {
                        Expression::Factor(val) => {
                            assert_eq!(*val, *expected_val);
                        }
                        _ => panic!("Expected Factor on right side"),
                    }
                }
                _ => panic!("Expected Operation2(Assign)"),
            },
            _ => panic!("Expected Statement::Expression"),
        }
    }
}

// 配列サイズが0以下の場合はエラー
#[test]
fn test_parse_array_declaration_invalid_size() {
    let tokens = vec![
        token_keyword_let(),
        token_ident("arr"),
        token_bracket_l(),
        token_number(0),
        token_bracket_r(),
        token_semicolon(),
    ];
    let (_stmts, errs) = parse_stmts(tokens);
    assert!(!errs.is_empty(), "Expected error for zero-size array");
}

// サイズ省略（[]）初期値から推論: let: arr[]([1, 2, 3]);
#[test]
fn test_parse_array_declaration_size_omitted_with_init() {
    let tokens = vec![
        token_keyword_let(),
        token_ident("arr"),
        token_bracket_l(),
        token_bracket_r(),
        token_paren_l(),
        token_bracket_l(),
        token_number(1),
        token_comma(),
        token_number(2),
        token_comma(),
        token_number(3),
        token_bracket_r(),
        token_paren_r(),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors, got: {:?}", errs);
    // 宣言 + 初期化式3つ = 4文
    assert_eq!(
        stmts.len(),
        4,
        "Expected 4 statements (1 declaration + 3 assignments)"
    );

    // 1つ目: 配列宣言（サイズ3と推論）
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, _, is_static, _, array_size) => {
            assert_eq!(name, "arr");
            assert_eq!(*is_static, false);
            assert_eq!(*array_size, Some(3), "Expected inferred size 3");
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }

    // 2-4つ目: 各要素への代入
    for (i, expected_val) in [1i64, 2, 3].iter().enumerate() {
        match &stmts[i + 1].statement {
            Statement::Expression(expr) => match &expr.expression {
                Expression::Operation2(Operator2::Assign, left, right) => {
                    match &left.expression {
                        Expression::ArrayAccess(name, index) => {
                            assert_eq!(name, "arr");
                            match &index.expression {
                                Expression::Factor(idx) => assert_eq!(*idx, i as i64),
                                _ => panic!("Expected Factor as index"),
                            }
                        }
                        _ => panic!("Expected ArrayAccess on left side"),
                    }
                    match &right.expression {
                        Expression::Factor(val) => assert_eq!(*val, *expected_val),
                        _ => panic!("Expected Factor on right side"),
                    }
                }
                _ => panic!("Expected Operation2(Assign)"),
            },
            _ => panic!("Expected Statement::Expression"),
        }
    }
}

fn token_string_literal(s: &str) -> PrettyToken {
    let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
    (Token::StringLiteral(chars), TokenInfo { code_pointer: 0 })
}

// サイズ省略 + 文字列初期化: let: str[]("ABC");
#[test]
fn test_parse_array_declaration_size_omitted_string() {
    let tokens = vec![
        token_keyword_let(),
        token_ident("str"),
        token_bracket_l(),
        token_bracket_r(),
        token_paren_l(),
        token_string_literal("ABC"),
        token_paren_r(),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors, got: {:?}", errs);
    // 宣言 + 'A', 'B', 'C', '\0' = 5文
    assert_eq!(stmts.len(), 5, "Expected 5 statements");

    // 1つ目: 配列宣言（サイズ4と推論: 3文字 + null）
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, _, is_static, _, array_size) => {
            assert_eq!(name, "str");
            assert_eq!(*is_static, false);
            assert_eq!(
                *array_size,
                Some(4),
                "Expected inferred size 4 (3 chars + null)"
            );
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
}

// エラー: '[]' でサイズ省略しているのに初期値なし: let: arr[];
#[test]
fn test_parse_array_declaration_size_omitted_no_init_error() {
    let tokens = vec![
        token_keyword_let(),
        token_ident("arr"),
        token_bracket_l(),
        token_bracket_r(),
        token_semicolon(),
    ];
    let (_stmts, errs) = parse_stmts(tokens);
    assert!(
        !errs.is_empty(),
        "Expected error for '[]' without initializer"
    );
}

// エラー: 空の初期化リスト: let: arr[]([]);
#[test]
fn test_parse_array_declaration_empty_init_error() {
    let tokens = vec![
        token_keyword_let(),
        token_ident("arr"),
        token_bracket_l(),
        token_bracket_r(),
        token_paren_l(),
        token_bracket_l(),
        token_bracket_r(),
        token_paren_r(),
        token_semicolon(),
    ];
    let (_stmts, errs) = parse_stmts(tokens);
    assert!(
        !errs.is_empty(),
        "Expected error for empty initializer list"
    );
}

#[test]
fn test_parse_empty_statements() {
    let tokens = vec![];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 0);
}

// Quality-5: trailing comma `func: f(x,)` はエラーとなること
#[test]
fn test_parse_func_trailing_comma_error() {
    // func: f(x,) {}
    let tokens = vec![
        token_keyword_func(),
        token_ident("f"),
        token_paren_l(),
        token_ident("x"),
        token_comma(),
        token_paren_r(),
        token_brace_l(),
        token_brace_r(),
    ];
    let (_stmts, errs) = parse_stmts(tokens);
    assert!(
        !errs.is_empty(),
        "Expected error for trailing comma in func args"
    );
}

// Quality-5: 先頭カンマ `func: f(,x)` はエラーとなること
#[test]
fn test_parse_func_leading_comma_error() {
    // func: f(,x) {}
    let tokens = vec![
        token_keyword_func(),
        token_ident("f"),
        token_paren_l(),
        token_comma(),
        token_ident("x"),
        token_paren_r(),
        token_brace_l(),
        token_brace_r(),
    ];
    let (_stmts, errs) = parse_stmts(tokens);
    assert!(
        !errs.is_empty(),
        "Expected error for leading comma in func args"
    );
}

// Quality-1: 配列サイズ 0 のエラー位置確認（エラーが記録されること）
#[test]
fn test_parse_array_zero_size_has_error() {
    // let: arr[0];
    let tokens = vec![
        token_keyword_let(),
        token_ident("arr"),
        token_bracket_l(),
        token_number(0),
        token_bracket_r(),
        token_semicolon(),
    ];
    let (_stmts, errs) = parse_stmts(tokens);
    assert!(!errs.is_empty(), "Expected error for zero-size array");
    // エラーメッセージが "array size must be positive" であること
    assert!(
        errs[0].message.contains("positive"),
        "Expected 'positive' in error message, got: {}",
        errs[0].message
    );
}

// static 変数宣言のテスト (Refactor-4: let/static 統合の確認)
#[test]
fn test_parse_static_variable() {
    // static: x;
    let tokens = vec![
        (
            Token::Keyword(Keyword::Static),
            TokenInfo { code_pointer: 0 },
        ),
        token_ident("x"),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, _, is_static, _, _) => {
            assert_eq!(name, "x");
            assert_eq!(
                *is_static, true,
                "Expected is_static = true for static declaration"
            );
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
}
