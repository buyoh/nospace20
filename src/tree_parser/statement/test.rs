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

fn token_keyword_struct() -> PrettyToken {
    (
        Token::Keyword(Keyword::Struct),
        TokenInfo { code_pointer: 0 },
    )
}

fn token_keyword_import() -> PrettyToken {
    (
        Token::Keyword(Keyword::Import),
        TokenInfo { code_pointer: 0 },
    )
}

fn token_keyword_weak() -> PrettyToken {
    (Token::Keyword(Keyword::Weak), TokenInfo { code_pointer: 0 })
}

fn token_keyword_export() -> PrettyToken {
    (
        Token::Keyword(Keyword::Export),
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

fn token_at() -> PrettyToken {
    (Token::At, TokenInfo { code_pointer: 0 })
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
        Statement::VariableDeclaration(name, expr, is_static, _, array_size, type_annot) => {
            assert_eq!(name, "x");
            assert_eq!(*is_static, false); // non-static
            assert_eq!(*array_size, None); // not an array
            assert!(type_annot.is_none());
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
        Statement::FunctionDeclaration(name, args, body, return_type) => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 0);
            assert_eq!(body.len(), 0);
            assert!(return_type.is_none());
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
        Statement::FunctionDeclaration(name, args, body, return_type) => {
            assert_eq!(name, "bar");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].0, "x");
            assert_eq!(body.len(), 0);
            assert!(return_type.is_none());
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
        Statement::FunctionDeclaration(name, args, body, return_type) => {
            assert_eq!(name, "baz");
            assert_eq!(args.len(), 2);
            assert_eq!(args[0].0, "x");
            assert_eq!(args[1].0, "y");
            assert_eq!(body.len(), 0);
            assert!(return_type.is_none());
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
        Statement::FunctionDeclaration(name, args, body, return_type) => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 0);
            assert_eq!(body.len(), 1);
            assert!(return_type.is_none());
            match &body[0].statement {
                Statement::Return(_) => (),
                _ => panic!("Expected Statement::Return in body"),
            }
        }
        _ => panic!("Expected Statement::FunctionDeclaration"),
    }
}

#[test]
fn test_parse_let_with_type_annotation() {
    // let: x@int;
    let tokens = vec![
        token_keyword_let(),
        token_ident("x"),
        token_at(),
        token_ident("int"),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::VariableDeclaration(name, _, _, _, _, type_annot) => {
            assert_eq!(name, "x");
            assert!(matches!(type_annot, Some(TypeSpec::Int)));
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
}

#[test]
fn test_parse_func_arg_and_return_type() {
    // func: f(x@int)@void {}
    let tokens = vec![
        token_keyword_func(),
        token_ident("f"),
        token_paren_l(),
        token_ident("x"),
        token_at(),
        token_ident("int"),
        token_paren_r(),
        token_at(),
        token_ident("void"),
        token_brace_l(),
        token_brace_r(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::FunctionDeclaration(name, args, body, return_type) => {
            assert_eq!(name, "f");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].0, "x");
            assert!(matches!(args[0].1, Some(TypeSpec::Int)));
            assert!(matches!(return_type, Some(TypeSpec::Void)));
            assert!(body.is_empty());
        }
        _ => panic!("Expected Statement::FunctionDeclaration"),
    }
}

#[test]
fn test_parse_struct_declaration() {
    // struct: Point(x, y@int);
    let tokens = vec![
        token_keyword_struct(),
        token_ident("Point"),
        token_paren_l(),
        token_ident("x"),
        token_comma(),
        token_ident("y"),
        token_at(),
        token_ident("int"),
        token_paren_r(),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::StructDeclaration(name, fields) => {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert!(fields[0].type_spec.is_none());
            assert_eq!(fields[1].name, "y");
            assert!(matches!(fields[1].type_spec, Some(TypeSpec::Int)));
        }
        _ => panic!("Expected Statement::StructDeclaration"),
    }
}

#[test]
fn test_parse_import_basic() {
    let tokens = vec![token_keyword_import(), token_ident("N1"), token_semicolon()];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::ImportDeclaration {
            namespace_name,
            is_weak,
            is_export,
        } => {
            assert_eq!(namespace_name, "N1");
            assert!(!is_weak);
            assert!(!is_export);
        }
        _ => panic!("Expected Statement::ImportDeclaration"),
    }
}

#[test]
fn test_parse_import_weak_export() {
    let tokens = vec![
        token_keyword_import(),
        token_keyword_weak(),
        token_keyword_export(),
        token_ident("N1"),
        token_semicolon(),
    ];
    let (stmts, errs) = parse_stmts(tokens);
    assert!(errs.is_empty(), "Expected no errors");
    assert_eq!(stmts.len(), 1);
    match &stmts[0].statement {
        Statement::ImportDeclaration {
            namespace_name,
            is_weak,
            is_export,
        } => {
            assert_eq!(namespace_name, "N1");
            assert!(*is_weak);
            assert!(*is_export);
        }
        _ => panic!("Expected Statement::ImportDeclaration"),
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
        Statement::VariableDeclaration(name, _, _, _, _, _) => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
    match &stmts[1].statement {
        Statement::VariableDeclaration(name, _, _, _, _, _) => {
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
        Statement::VariableDeclaration(name, expr, is_static, _, array_size, type_annot) => {
            assert_eq!(name, "arr");
            assert_eq!(*is_static, false);
            assert_eq!(*array_size, Some(4));
            assert!(type_annot.is_none());
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
    // 初期化リストは宣言の初期化子（ArrayInitializer）になり、要素の代入文は作らない
    assert_eq!(stmts.len(), 1, "Expected 1 declaration statement");
    assert_array_initializer_declaration(&stmts[0], "arr", 3, &[10, 20, 30]);
}

/// `name = ArrayInitializer(values)` を初期化に持つ配列宣言（非 static）であることを確かめる
fn assert_array_initializer_declaration(
    stmt: &LocatedStatement,
    expected_name: &str,
    expected_size: i64,
    expected_values: &[i64],
) {
    match &stmt.statement {
        Statement::VariableDeclaration(name, init, is_static, _, array_size, type_annot) => {
            assert_eq!(name, expected_name);
            assert!(!*is_static);
            assert_eq!(*array_size, Some(expected_size));
            assert!(type_annot.is_none());
            match &init.expression {
                Expression::Operation2(Operator2::Assign, left, right) => {
                    match &left.expression {
                        Expression::Variable(target) => assert_eq!(target, expected_name),
                        _ => panic!("Expected Variable on left side"),
                    }
                    match &right.expression {
                        Expression::ArrayInitializer(values) => {
                            let actual: Vec<i64> = values
                                .iter()
                                .map(|v| match v.expression {
                                    Expression::Factor(n) => n,
                                    _ => panic!("Expected Factor as element"),
                                })
                                .collect();
                            assert_eq!(actual, expected_values);
                        }
                        _ => panic!("Expected ArrayInitializer on right side"),
                    }
                }
                _ => panic!("Expected Operation2(Assign) as initializer"),
            }
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
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
    // サイズ 3 と推論され、初期化リストは宣言の初期化子になる
    assert_eq!(stmts.len(), 1, "Expected 1 declaration statement");
    assert_array_initializer_declaration(&stmts[0], "arr", 3, &[1, 2, 3]);
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
    // サイズ 4（3 文字 + ヌル終端）と推論され、文字コードと末尾の 0 が宣言の初期化子になる
    assert_eq!(stmts.len(), 1, "Expected 1 declaration statement");
    assert_array_initializer_declaration(&stmts[0], "str", 4, &[65, 66, 67, 0]);
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
        Statement::VariableDeclaration(name, _, is_static, _, _, _) => {
            assert_eq!(name, "x");
            assert_eq!(
                *is_static, true,
                "Expected is_static = true for static declaration"
            );
        }
        _ => panic!("Expected Statement::VariableDeclaration"),
    }
}
