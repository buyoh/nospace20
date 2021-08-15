use std::collections::BTreeMap;

use crate::{
    syntactic_analyzer::Scope,
    tree_parser::{Expression, Operator2, Statement},
};

struct Environment<'a> {
    root_scope: &'a Scope,
    current_scope: &'a Scope,
    variables: BTreeMap<String, i64>,
}

impl Environment<'_> {
    fn interpret_call_function(&mut self, id: &String, args: &Vec<Box<Expression>>) -> i64 {
        let func = self.root_scope.get_function(&id.to_string()).unwrap();
        let mut variables = BTreeMap::<String, i64>::new();
        for id_eval in func.args.iter().zip(args.iter()) {
            variables.insert(id_eval.0.clone(), self.interpret_expression(id_eval.1));
        }
        let mut e = Environment {
            root_scope: self.root_scope,
            current_scope: &func.scope,
            variables,
        };
        e.interpret_statements(&func.code);

        0
    }

    fn interpret_expression(&mut self, expr: &Box<Expression>) -> i64 {
        match expr.as_ref() {
            Expression::Operation2(op, left, right) => match op {
                Operator2::Plus => {
                    self.interpret_expression(left) + self.interpret_expression(right)
                }
                Operator2::Minus => {
                    self.interpret_expression(left) - self.interpret_expression(right)
                }
                Operator2::Multiply => {
                    self.interpret_expression(left) * self.interpret_expression(right)
                }
                Operator2::Divide => {
                    self.interpret_expression(left) / self.interpret_expression(right)
                }
                Operator2::Assign => {
                    if let Expression::Variable(name) = left.as_ref() {
                        if self.variables.contains_key(name) {
                            // todo: more nice impl
                            // todo: should be checked not in runtime.
                            let v = self.interpret_expression(right);
                            self.variables.insert(name.clone(), v);
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
                // if id == "pow2" {
                //     let a = self.interpret_expression(args.first().unwrap());
                //     a * a
                // } else {
                //     panic!("syntax error: unknown identifier")
                // }
                self.interpret_call_function(id, args)
            }
            Expression::Factor(v) => *v,
            Expression::Variable(name) => {
                if let Some(val) = self.variables.get(name) {
                    *val
                } else {
                    panic!("syntax error: unknown variable name")
                }
            }
            Expression::Invalid(_) => todo!(),
        }
    }

    fn interpret_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::VariableDeclaration(name, expr) => {
                if self.variables.contains_key(name) {
                    panic!("runtime error: (internal error) redeclaration");
                }
                let v = self.interpret_expression(&expr);
                self.variables.insert(name.clone(), v);
            }
            Statement::FunctionDeclaration(_, _, _) => todo!(),
            Statement::Expression(expr) => {
                self.interpret_expression(&expr);
            }
            Statement::Return(expr) => {
                // this is not return :p
                let x = self.interpret_expression(&expr);
                println!("ans = {}", x);
            }
        }
    }

    pub fn interpret_statements(&mut self, statements: &Vec<Statement>) {
        for statement in statements {
            self.interpret_statement(statement);
        }
    }
}

pub fn interpret_main_func(scope: &Scope) {
    let func = scope.get_function(&"main".to_string()).unwrap();
    let mut e = Environment {
        root_scope: scope,
        current_scope: &func.scope,
        variables: BTreeMap::<String, i64>::new(),
    };
    e.interpret_statements(&func.code);
}
