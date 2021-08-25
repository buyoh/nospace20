use std::collections::BTreeMap;

use crate::{
    syntactic_analyzer::{ExecStatement, Function, Scope},
    tree_parser::{Expression, Operator2},
};

enum Flow {
    Proceed,
    Return(i64),
}

struct Environment<'a> {
    root_scope: &'a Scope,
    current_scope: &'a Scope,
    variables: BTreeMap<String, i64>,
}

impl Environment<'_> {
    fn new_func<'a>(
        root_scope: &'a Scope,
        func: &'a Function,
        args: &mut dyn std::iter::Iterator<Item = i64>, // TODO: mut?
    ) -> Environment<'a> {
        let mut variables = BTreeMap::<String, i64>::new();
        for id_eval in func.args.iter().zip(args) {
            variables.insert(id_eval.0.clone(), id_eval.1);
        }
        for v in func.scope.variables.iter() {
            if !variables.contains_key(&v.identifier) {
                variables.insert(v.identifier.clone(), 0);
            }
        }
        Environment {
            root_scope,
            current_scope: &func.scope,
            variables,
        }
    }

    fn interpret_call_function(&mut self, id: &String, args: &Vec<Box<Expression>>) -> i64 {
        let func = self.root_scope.get_function(&id.to_string()).unwrap();
        let mut e = Environment::new_func(
            self.root_scope,
            &func,
            &mut args.iter().map(|e| self.interpret_expression(e)),
        );
        e.interpret_statements(&func.code)
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
                            panic!("syntax error: unknown variable name `{}`", name)
                        }
                    } else {
                        panic!("runtime error: left value is not variable");
                    }
                }
            },
            Expression::Function(id, args) => {
                if id == "__clog" {
                    let a = self.interpret_expression(args.first().unwrap());
                    println!("__clog: {}", a);
                    a
                } else {
                    self.interpret_call_function(id, args)
                }
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

    fn interpret_statement(&mut self, statement: &ExecStatement) -> Flow {
        match statement {
            ExecStatement::Expression(expr) => {
                self.interpret_expression(&expr);
                Flow::Proceed
            }
            ExecStatement::Return(expr) => {
                // this is not return :p
                let x = self.interpret_expression(&expr);
                Flow::Return(x)
            }
        }
    }

    pub fn interpret_statements(&mut self, statements: &Vec<ExecStatement>) -> i64 {
        for statement in statements {
            match self.interpret_statement(statement) {
                Flow::Proceed => (),
                Flow::Return(x) => return x,
            }
        }
        0
    }
}

pub fn interpret_main_func(scope: &Scope) {
    let func = scope.get_function(&"main".to_string()).unwrap();
    let mut e = Environment::new_func(scope, &func, &mut Vec::<i64>::new().into_iter());
    let res = e.interpret_statements(&func.code);
    println!("main returns: {}", res);
}
