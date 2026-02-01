//! # Converter
//!
//! ASTから実行可能な中間表現への変換を行う。

use crate::tree_parser::Expression;

use super::types::{ExecExpression, ExecStatement, Function, ScopeBuilder, ScopeType, Variable};

pub(crate) fn convert_to_exec_expression(expr: Box<Expression>) -> Box<ExecExpression> {
    match *expr {
        Expression::Operation1(op, x) => Box::new(ExecExpression::Operation1(
            op,
            convert_to_exec_expression(x),
        )),
        Expression::Operation2(op, l, r) => Box::new(ExecExpression::Operation2(
            op,
            convert_to_exec_expression(l),
            convert_to_exec_expression(r),
        )),
        Expression::If(cond, stat1, stat2) => Box::new(ExecExpression::If(
            convert_to_exec_expression(cond),
            analyze_internal(&stat1, ScopeType::Block).1,
            analyze_internal(&stat2, ScopeType::Block).1,
        )),
        Expression::While(expr, stat) => Box::new(ExecExpression::While(
            convert_to_exec_expression(expr),
            analyze_internal(&stat, ScopeType::Block).1,
        )),
        Expression::Function(f, a) => Box::new(ExecExpression::Function(
            f,
            a.into_iter().map(|e| convert_to_exec_expression(e)).collect(),
        )),
        Expression::Factor(v) => Box::new(ExecExpression::Factor(v)),
        Expression::Variable(v) => Box::new(ExecExpression::Variable(v)),
        // パースエラー時のみ Invalid が生成されるため、正常系では到達しない
        Expression::Invalid(_) => unreachable!("Expression::Invalid should not reach semantic analysis"),
    }
}

pub(crate) fn convert_to_exec_statement(
    stat: &crate::tree_parser::Statement,
    scope_type: &ScopeType,
    exec_statements: &mut Vec<ExecStatement>,
    scope: &mut ScopeBuilder,
) {
    use crate::tree_parser::Statement;
    
    match stat {
        Statement::VariableDeclaration(name, init) => {
            if let ScopeType::Block = scope_type {
                // TODO(unimplemented): ブロックスコープ変数は未実装
                panic!("todo: block scoped variable is not implemented")
            }
            if let ScopeType::Root = scope_type {
                // TODO(unimplemented): グローバル変数は未実装
                panic!("todo: global variable is not implemented")
            }
            scope.add_variable(
                name.clone(),
                Variable {
                    identifier: name.clone(),
                },
            );
            exec_statements.push(ExecStatement::Expression(convert_to_exec_expression(init.clone())));
        }
        Statement::FunctionDeclaration(name, args, block) => {
            if !matches!(scope_type, ScopeType::Root) {
                // TODO(error-handling): Result型でエラーを返すべき (ネスト関数宣言は未対応)
                panic!("semantic error: nested function declaration is not supported")
            }
            let (mut s, es) = analyze_internal(block, ScopeType::Function);
            // add variable definition to scope
            for a in args {
                s.add_variable(
                    a.clone(),
                    Variable {
                        identifier: a.clone(),
                    },
                );
            }
            // store variable identifier to function
            let func = Function {
                args: args.clone(),
                scope: s.build(),
                code: es,
            };
            scope.add_function(name.clone(), func);
        }
        Statement::Return(e) => {
            if let ScopeType::Root = scope_type {
                // TODO(error-handling): Result型でエラーを返すべき
                panic!("semantic error: return statement outside of function")
            }
            exec_statements.push(ExecStatement::Return(convert_to_exec_expression(e.clone())));
        }
        Statement::Expression(e) => {
            if let ScopeType::Root = scope_type {
                // TODO(error-handling): Result型でエラーを返すべき
                panic!("semantic error: expression statement at root level")
            }
            exec_statements.push(ExecStatement::Expression(convert_to_exec_expression(e.clone())));
        }
        Statement::Continue => {
            if let ScopeType::Root = scope_type {
                // TODO(error-handling): Result型でエラーを返すべき
                panic!("semantic error: continue statement outside of function")
            }
            exec_statements.push(ExecStatement::Continue);
        }
        Statement::Break => {
            if let ScopeType::Root = scope_type {
                // TODO(error-handling): Result型でエラーを返すべき
                panic!("semantic error: break statement outside of function")
            }
            exec_statements.push(ExecStatement::Break);
        }
        Statement::Invalid(_) => (),
    }
}

pub(crate) fn analyze_internal(
    statements: &Vec<crate::tree_parser::Statement>,
    scope_type: ScopeType,
) -> (ScopeBuilder, Vec<ExecStatement>) {
    let mut scope = ScopeBuilder::new();
    let mut exec_statements = Vec::<ExecStatement>::new();
    for stat in statements {
        convert_to_exec_statement(stat, &scope_type, &mut exec_statements, &mut scope);
    }
    (scope, exec_statements)
}
