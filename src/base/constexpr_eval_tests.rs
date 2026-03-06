use super::*;
use crate::base::SourceLocation;
use crate::tree_parser::{LocatedExpression, LocatedStatement};

fn dummy_loc() -> SourceLocation {
    SourceLocation::from_single(0)
}

fn make_factor(n: i64) -> Box<LocatedExpression> {
    Box::new(LocatedExpression {
        expression: Expression::Factor(n),
        location: dummy_loc(),
    })
}

fn make_variable(name: &str) -> Box<LocatedExpression> {
    Box::new(LocatedExpression {
        expression: Expression::Variable(name.to_string()),
        location: dummy_loc(),
    })
}

fn make_op2(
    op: Operator2,
    l: Box<LocatedExpression>,
    r: Box<LocatedExpression>,
) -> Box<LocatedExpression> {
    Box::new(LocatedExpression {
        expression: Expression::Operation2(op, l, r),
        location: dummy_loc(),
    })
}

fn empty_table() -> BTreeMap<String, i64> {
    BTreeMap::new()
}

#[test]
fn test_eval_expr_factor() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let expr = make_factor(42);
    assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 42);
}

#[test]
fn test_eval_expr_arithmetic() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    // 3 + 4 = 7
    let expr = make_op2(Operator2::Plus, make_factor(3), make_factor(4));
    assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 7);
    // 10 * 2 = 20
    let expr2 = make_op2(Operator2::Multiply, make_factor(10), make_factor(2));
    assert_eq!(eval_constexpr_expr(&expr2, &mut env).unwrap(), 20);
    // 10 / 0 => error
    let expr3 = make_op2(Operator2::Divide, make_factor(10), make_factor(0));
    assert!(eval_constexpr_expr(&expr3, &mut env).is_err());
}

#[test]
fn test_eval_expr_variable() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    env.declare_variable("x".to_string(), 99);
    let expr = make_variable("x");
    assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 99);
}

#[test]
fn test_eval_expr_constexpr_ref() {
    let mut table = BTreeMap::new();
    table.insert("CONST".to_string(), 123i64);
    let mut env = ConstexprEnv::new(&table);
    let expr = make_variable("CONST");
    assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 123);
}

#[test]
fn test_eval_expr_undefined_variable() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let expr = make_variable("undefined");
    assert!(eval_constexpr_expr(&expr, &mut env).is_err());
}

fn make_let_stmt(name: &str, value: i64) -> LocatedStatement {
    LocatedStatement {
        statement: Statement::VariableDeclaration(
            name.to_string(),
            make_factor(value),
            false,
            false,
            None,
        ),
        location: dummy_loc(),
    }
}

fn make_expr_stmt(expr: Box<LocatedExpression>) -> LocatedStatement {
    LocatedStatement {
        statement: Statement::Expression(expr),
        location: dummy_loc(),
    }
}

#[test]
fn test_eval_block_let() {
    // { let: x(5); x; }  => 5
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let stmts = vec![make_let_stmt("x", 5), make_expr_stmt(make_variable("x"))];
    assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 5);
}

#[test]
fn test_eval_block_assign() {
    // { let: x(1); x = 10; x; } => 10
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let assign_expr = make_op2(Operator2::Assign, make_variable("x"), make_factor(10));
    let stmts = vec![
        make_let_stmt("x", 1),
        make_expr_stmt(assign_expr),
        make_expr_stmt(make_variable("x")),
    ];
    assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 10);
}

#[test]
fn test_eval_block_compound_assign() {
    // { let: x(3); x += 7; x; } => 10
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let plus_assign = make_op2(Operator2::PlusAssign, make_variable("x"), make_factor(7));
    let stmts = vec![
        make_let_stmt("x", 3),
        make_expr_stmt(plus_assign),
        make_expr_stmt(make_variable("x")),
    ];
    assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 10);
}

#[test]
fn test_eval_block_if() {
    // if 式: 条件真
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    // if(1) { 42; } else { 0; } => 42
    let then_stmts = vec![make_expr_stmt(make_factor(42))];
    let else_stmts = vec![make_expr_stmt(make_factor(0))];
    let if_expr = Box::new(LocatedExpression {
        expression: Expression::If(make_factor(1), then_stmts.clone(), else_stmts.clone()),
        location: dummy_loc(),
    });
    let stmts = vec![make_expr_stmt(if_expr)];
    assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 42);

    // 条件偽
    let mut env2 = ConstexprEnv::new(&table);
    let if_expr2 = Box::new(LocatedExpression {
        expression: Expression::If(make_factor(0), then_stmts, else_stmts),
        location: dummy_loc(),
    });
    let stmts2 = vec![make_expr_stmt(if_expr2)];
    assert_eq!(eval_constexpr_block(&stmts2, &mut env2).unwrap(), 0);
}

#[test]
fn test_eval_block_nested_scope() {
    // { let: x(1); { let: x(2); x; }; x; }  => 外のxは1のまま
    // ネストしたBlockは式として評価
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let inner_stmts = vec![make_let_stmt("x", 2), make_expr_stmt(make_variable("x"))];
    let block_expr = Box::new(LocatedExpression {
        expression: Expression::Block(inner_stmts),
        location: dummy_loc(),
    });
    let stmts = vec![
        make_let_stmt("x", 1),
        make_expr_stmt(block_expr),
        make_expr_stmt(make_variable("x")),
    ];
    // 最後の式は x = 1
    assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 1);
}

#[test]
fn test_eval_block_no_value_error() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    // 空のブロックはエラー
    assert!(eval_constexpr_block(&[], &mut env).is_err());
}

#[test]
fn test_eval_block_static_error() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let stmts = vec![LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            make_factor(1),
            true, // is_static=true
            false,
            None,
        ),
        location: dummy_loc(),
    }];
    assert!(eval_constexpr_block(&stmts, &mut env).is_err());
}

#[test]
fn test_eval_block_final_error() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let stmts = vec![LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            make_factor(1),
            false,
            true, // is_final=true
            None,
        ),
        location: dummy_loc(),
    }];
    assert!(eval_constexpr_block(&stmts, &mut env).is_err());
}

#[test]
fn test_eval_block_array_error() {
    let table = empty_table();
    let mut env = ConstexprEnv::new(&table);
    let stmts = vec![LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            make_factor(0),
            false,
            false,
            Some(10), // array_size=Some(10)
        ),
        location: dummy_loc(),
    }];
    assert!(eval_constexpr_block(&stmts, &mut env).is_err());
}

#[test]
fn test_eval_assign_to_constexpr_table_error() {
    // constexpr テーブルの定数への代入はエラー
    let mut table = BTreeMap::new();
    table.insert("CONST".to_string(), 42i64);
    let mut env = ConstexprEnv::new(&table);
    let assign = make_op2(Operator2::Assign, make_variable("CONST"), make_factor(1));
    let stmts = vec![make_expr_stmt(assign)];
    assert!(eval_constexpr_block(&stmts, &mut env).is_err());
}
