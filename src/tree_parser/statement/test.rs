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
    let tokens = vec![
        token_keyword_let(),
        token_colon(),
        token_ident("x"),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, expr, is_static) => {
            assert_eq!(name, "x");
            assert_eq!(*is_static, false); // non-static
            match **expr {
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
    let tokens = vec![
        token_keyword_return(),
        token_colon(),
        token_number(42),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::Return(expr) => match **expr {
            Expression::Factor(42) => (),
            _ => panic!("Expected Factor(42)"),
        },
        _ => panic!("Expected Statement::Return"),
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
        Statement::Expression(expr) => match **expr {
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
        token_colon(),
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
        token_colon(),
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
        token_colon(),
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
        token_colon(),
        token_ident("foo"),
        token_paren_l(),
        token_paren_r(),
        token_brace_l(),
        token_keyword_return(),
        token_colon(),
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
        token_colon(),
        token_ident("x"),
        token_semicolon(),
        token_keyword_let(),
        token_colon(),
        token_ident("y"),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 2);
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, _, _) => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
    match &stmts[1].statement {
        Statement::VariableDeclaration(name, _, _) => {
            assert_eq!(name, "y");
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
}

#[test]
fn test_parse_empty_statements() {
    let tokens = vec![];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 0);
}
