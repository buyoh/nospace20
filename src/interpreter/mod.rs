use std::collections::BTreeMap;

use crate::tree_parser::{Expression, Operator2, Statement};

fn interpret_expression(expr: &Box<Expression>, variables: &mut BTreeMap<String, i64>) -> i64 {
    match expr.as_ref() {
        Expression::Operation2(op, left, right) => match op {
            Operator2::Plus => {
                interpret_expression(left, variables) + interpret_expression(right, variables)
            }
            Operator2::Minus => {
                interpret_expression(left, variables) - interpret_expression(right, variables)
            }
            Operator2::Multiply => {
                interpret_expression(left, variables) * interpret_expression(right, variables)
            }
            Operator2::Divide => {
                interpret_expression(left, variables) / interpret_expression(right, variables)
            }
            Operator2::Assign => {
                if let Expression::Variable(name) = left.as_ref() {
                    if variables.contains_key(name) {
                        // todo: more nice impl
                        // todo: should be checked not in runtime.
                        let v = interpret_expression(right, variables);
                        variables.insert(name.clone(), v);
                        v
                    } else {
                        panic!("syntax error: unknown variable name")
                    }
                } else {
                    panic!("runtime error: left value is not variable");
                }
            }
        },
        Expression::Function(id, args) => {
            if id == "pow2" {
                let a = interpret_expression(args.first().unwrap(), variables);
                a * a
            } else {
                panic!("syntax error: unknown identifier")
            }
        }
        Expression::Factor(v) => *v,
        Expression::Variable(name) => {
            if let Some(val) = variables.get(name) {
                *val
            } else {
                panic!("syntax error: unknown variable name")
            }
        }
    }
}

fn interpret_statement(statement: &Statement, variables: &mut BTreeMap<String, i64>) {
    match statement {
        Statement::VariableDeclaration(name, expr) => {
            if variables.contains_key(name) {
                panic!("runtime error: redeclaration");
            }
            let v = interpret_expression(&expr, variables);
            variables.insert(name.clone(), v);
        }
        Statement::Expression(expr) => {
            let x = interpret_expression(&expr, variables);
            println!("ans = {}", x);
        }
    }
}

pub fn interpret_statements(statements: &Vec<Statement>) {
    let mut variables = BTreeMap::<String, i64>::new();
    for statement in statements {
        interpret_statement(statement, &mut variables);
    }
}
