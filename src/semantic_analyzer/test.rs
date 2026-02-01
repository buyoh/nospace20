//! # Semantic Analyzer Tests
//!
//! semantic_analyzer モジュールのユニットテスト

use crate::tree_parser::{Expression, Statement, Operator2};
use super::types::{ExecExpression, ExecStatement};
use super::converter;

// ========================================
// Test Helpers
// ========================================

/// 数値式を作成するヘルパー
pub fn make_number_expr(value: i64) -> Expression {
    Expression::Factor(value)
}

/// 変数式を作成するヘルパー
pub fn make_variable_expr(name: &str) -> Expression {
    Expression::Variable(name.to_string())
}

/// 二項演算式を作成するヘルパー
pub fn make_binary_expr(op: Operator2, left: Expression, right: Expression) -> Expression {
    Expression::Operation2(
        op,
        Box::new(left),
        Box::new(right),
    )
}

/// 変数宣言文を作成するヘルパー
pub fn make_var_decl(name: &str, init: Expression) -> Statement {
    Statement::VariableDeclaration(name.to_string(), Box::new(init))
}

/// 関数宣言文を作成するヘルパー
pub fn make_function(name: &str, args: Vec<&str>, body: Vec<Statement>) -> Statement {
    Statement::FunctionDeclaration(
        name.to_string(),
        args.iter().map(|s| s.to_string()).collect(),
        body,
    )
}

/// return文を作成するヘルパー
pub fn make_return(expr: Expression) -> Statement {
    Statement::Return(Box::new(expr))
}

/// 式文を作成するヘルパー
pub fn make_expr_statement(expr: Expression) -> Statement {
    Statement::Expression(Box::new(expr))
}

// ========================================
// Tests
// ========================================

#[test]
fn test_analyze_simple_function() {
    // fn main() {}
    let statements = vec![make_function("main", vec![], vec![])];
    
    let scope = super::analyze(&statements);
    
    assert!(scope.get_function("main").is_some());
    let func = scope.get_function("main").unwrap();
    assert_eq!(func.args.len(), 0);
    assert_eq!(func.code.len(), 0);
}

#[test]
fn test_analyze_function_with_args() {
    // fn add(a, b) { return a + b }
    let statements = vec![make_function(
        "add",
        vec!["a", "b"],
        vec![make_return(make_binary_expr(
            Operator2::Plus,
            make_variable_expr("a"),
            make_variable_expr("b"),
        ))],
    )];
    
    let scope = super::analyze(&statements);
    
    assert!(scope.get_function("add").is_some());
    let func = scope.get_function("add").unwrap();
    assert_eq!(func.args.len(), 2);
    assert_eq!(func.args[0], "a");
    assert_eq!(func.args[1], "b");
    
    // スコープ内に引数が変数として定義されているか確認
    assert!(func.scope.get_variable("a").is_some());
    assert!(func.scope.get_variable("b").is_some());
}

#[test]
fn test_analyze_variable_decl() {
    // fn test() { var x = 42 }
    let statements = vec![make_function(
        "test",
        vec![],
        vec![make_var_decl("x", make_number_expr(42))],
    )];
    
    let scope = super::analyze(&statements);
    let func = scope.get_function("test").unwrap();
    
    // 変数が定義されているか確認
    assert!(func.scope.get_variable("x").is_some());
    let var = func.scope.get_variable("x").unwrap();
    assert_eq!(var.identifier, "x");
}

#[test]
fn test_analyze_multiple_functions() {
    // fn f1() {} fn f2() {}
    let statements = vec![
        make_function("f1", vec![], vec![]),
        make_function("f2", vec![], vec![]),
    ];
    
    let scope = super::analyze(&statements);
    
    assert!(scope.get_function("f1").is_some());
    assert!(scope.get_function("f2").is_some());
}

#[test]
fn test_convert_number_expression() {
    let expr = Box::new(make_number_expr(123));
    let exec_expr = converter::convert_to_exec_expression(expr.clone());
    
    match *exec_expr {
        ExecExpression::Factor(value) => assert_eq!(value, 123),
        _ => panic!("Expected Factor variant"),
    }
}

#[test]
fn test_convert_variable_expression() {
    let expr = Box::new(make_variable_expr("x"));
    let exec_expr = converter::convert_to_exec_expression(expr.clone());
    
    match *exec_expr {
        ExecExpression::Variable(ref name) => assert_eq!(name, "x"),
        _ => panic!("Expected Variable variant"),
    }
}

#[test]
fn test_convert_binary_expression() {
    let expr = Box::new(make_binary_expr(
        Operator2::Plus,
        make_number_expr(1),
        make_number_expr(2),
    ));
    let exec_expr = converter::convert_to_exec_expression(expr.clone());
    
    match *exec_expr {
        ExecExpression::Operation2(ref op, ref left, ref right) => {
            assert!(matches!(op, Operator2::Plus));
            match **left {
                ExecExpression::Factor(v) => assert_eq!(v, 1),
                _ => panic!("Expected Factor in left"),
            }
            match **right {
                ExecExpression::Factor(v) => assert_eq!(v, 2),
                _ => panic!("Expected Factor in right"),
            }
        }
        _ => panic!("Expected Operation2 variant"),
    }
}

#[test]
#[should_panic(expected = "the name is already used")]
fn test_error_duplicate_function() {
    // fn f() {} fn f() {}
    let statements = vec![
        make_function("f", vec![], vec![]),
        make_function("f", vec![], vec![]),
    ];
    
    super::analyze(&statements);
}

#[test]
#[should_panic(expected = "nested function declaration is not supported")]
fn test_error_nested_function() {
    // fn outer() { fn inner() {} }
    let statements = vec![make_function(
        "outer",
        vec![],
        vec![make_function("inner", vec![], vec![])],
    )];
    
    super::analyze(&statements);
}

#[test]
#[should_panic(expected = "return statement outside of function")]
fn test_error_return_at_root() {
    // return 42 (at root level)
    let statements = vec![make_return(make_number_expr(42))];
    
    super::analyze(&statements);
}

#[test]
fn test_analyze_recursive_call() {
    // fn factorial(n) { return factorial(n) }
    let statements = vec![make_function(
        "factorial",
        vec!["n"],
        vec![make_return(Expression::Function(
            "factorial".to_string(),
            vec![Box::new(make_variable_expr("n"))],
        ))],
    )];
    
    let scope = super::analyze(&statements);
    let func = scope.get_function("factorial").unwrap();
    
    // 関数が定義されているか確認
    assert_eq!(func.args.len(), 1);
    assert_eq!(func.args[0], "n");
    
    // return文が含まれているか確認
    assert_eq!(func.code.len(), 1);
    match &func.code[0] {
        ExecStatement::Return(expr) => match **expr {
            ExecExpression::Function(ref name, ref args) => {
                assert_eq!(name, "factorial");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Function call in return"),
        },
        _ => panic!("Expected Return statement"),
    }
}
