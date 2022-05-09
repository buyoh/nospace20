//! # Interpreter
//!
//! コンパイル前のコードを実行します。
//! コンパイラの実装は多様で複雑になりがちな為、Interpreterは極力シンプルな実装となるよう
//! 他のモジュールを設計しなければなりません。
//!

use std::collections::BTreeMap;

use crate::{
    syntactic_analyzer::{Entity, ExecExpression, ExecStatement, Function, Identifier, Scope},
    tree_parser::{Operator1, Operator2},
};

// Block(Vec<Statement>) の評価結果
enum Flow {
    Proceed,
    Return(i64),
    Continue,
    Break,
}

// Expression の評価結果
enum ExpressionFlow {
    Value(i64),
    Jump(Flow),
}

macro_rules! try_expr {
    ($e: expr) => {
        match $e {
            ExpressionFlow::Value(x) => x,
            ExpressionFlow::Jump(f) => return ExpressionFlow::Jump(f),
        }
    };
}

pub struct Environment {
    pub traced: BTreeMap<i64, i64>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            traced: BTreeMap::new(),
        }
    }
}

// 1つのfunction scopeの`実行時インスタンス`を管理する
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    current_scopes: Vec<&'a Scope>, // 末尾ほど、深いブロックスコープ。先頭は必ず関数スコープ。先頭以外は必ずブロックスコープ
    parent_scope: Option<&'a Scope>, // Scope が親の情報を持っていないので、ここで持つ
    parent_env: Option<&'a LocalEnvironment<'a, 'aenv>>,
    variables: BTreeMap<Identifier, i64>,
}

fn bool_to_int(x: bool) -> i64 {
    if x {
        1
    } else {
        0
    }
}

impl LocalEnvironment<'_, '_> {
    fn new_func<'a, 'aenv>(
        env: &'aenv mut Environment,
        root_scope: &'a Scope,
        func: &'a Function,
        args: &Vec<i64>,
    ) -> LocalEnvironment<'a, 'aenv> {
        let mut variables = BTreeMap::<Identifier, i64>::new();
        for id in func
            .block
            .scope
            .iter_identifier()
            .filter_map(|(i, e)| match e {
                Entity::Function(_) => None,
                Entity::Variable(_) => Some(i),
            })
        {
            variables.insert(id.clone(), 0);
        }
        for id_eval in func.args.iter().zip(args) {
            variables.insert(id_eval.0.clone(), *id_eval.1);
        }
        // - 関数を呼び出して、関数内で変数にアクセスするとする。
        // 呼び出せる変数は、「親のスコープにあるstatic変数(未実装)」直近のLocalEnvironmentだけ
        // 注意しなければならないのは、親のスコープにある変数を呼び出せること。LocalEnvironmentではない。
        // LocalEnvironment はスタックであり、LocalEnvironmentに積まれている値を取り出せるわけではい。
        LocalEnvironment {
            env,
            root_scope,
            current_scope: &func.block.scope,
            variables,
        }
    } // TODO: static スコープ作ったほうが良いかも

    fn get_variable(&self, id: &Identifier) -> Option<&mut i64> {
        // TODO: だから、以下の実装は間違い。親のスコープを参照しなければならない。
        if let Some(a) = self.variables.get_mut(id) {
            Some(a)
        } else if let Some(env) = self.parent_env {
            if let Some(a) = env.get_variable(id) {
                Some(a)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn interpret_call_function(
        &mut self,
        id: &String,
        args: &Vec<Box<ExecExpression>>,
    ) -> ExpressionFlow {
        match id.as_str() {
            "__clog" => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                println!("__clog: {}", a);
                ExpressionFlow::Value(a)
            }
            "__assert" => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                if a == 0 {
                    // TODO: 気の利いたログを出せない
                    panic!("assertion failed: {} == 0", a);
                }
                ExpressionFlow::Value(a)
            }
            "__assert_not" => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                if a != 0 {
                    // TODO: 気の利いたログを出せない
                    panic!("assertion failed: {} != 0", a);
                }
                ExpressionFlow::Value(a)
            }
            "__trace" => {
                // TODO: 未だ比較演算子を実装していないので not
                let key = try_expr!(self.interpret_expression(args.first().unwrap()));
                let traced = &mut self.env.traced;
                if let Some(v) = traced.get_mut(&key) {
                    *v += 1;
                } else {
                    traced.insert(key, 1);
                }
                ExpressionFlow::Value(0)
            }
            _ => self.interpret_call_user_function(id, args),
        }
    }

    fn interpret_call_user_function(
        &mut self,
        id: &String,
        args: &Vec<Box<ExecExpression>>,
    ) -> ExpressionFlow {
        let mut arg_values = Vec::new();
        arg_values.reserve(args.len());
        for a in args {
            // note: We can't use `map` because some args may say `return`/`break`;
            arg_values.push(try_expr!(self.interpret_expression(a)));
        }
        let func = self.root_scope.get_function(id.as_str()).unwrap();

        let mut env = LocalEnvironment::new_func(self.env, self.root_scope, &func, &arg_values);
        match env.interpret_statements(&func.code) {
            Flow::Proceed => ExpressionFlow::Value(0),
            Flow::Continue => panic!("internal error: unexpected continue"),
            Flow::Break => panic!("internal error: unexpected break"),
            other => ExpressionFlow::Jump(other),
        }
    }

    fn interpret_while(
        &mut self,
        cond: &Box<ExecExpression>,
        code: &Vec<ExecStatement>,
    ) -> ExpressionFlow {
        loop {
            let cond = match self.interpret_expression(cond) {
                ExpressionFlow::Value(e) => e,
                ExpressionFlow::Jump(Flow::Return(x)) => {
                    return ExpressionFlow::Jump(Flow::Return(x))
                }
                // TODO: exclude on comile-time.
                ExpressionFlow::Jump(Flow::Continue) => panic!(
                    "internal error: unexpected continue: Don't call continue in `while` condition"
                ),
                ExpressionFlow::Jump(Flow::Break) => panic!(
                    "internal error: unexpected break: Don't call break in `while` condition"
                ),
                ExpressionFlow::Jump(Flow::Proceed) => {
                    panic!("internal error: unexpected Flow::Proceed")
                }
            };
            if cond == 0 {
                break;
            }
            match self.interpret_statements(code) {
                Flow::Proceed => (),
                Flow::Return(v) => return ExpressionFlow::Value(v),
                Flow::Continue => continue,
                Flow::Break => break,
            }
        }
        ExpressionFlow::Value(0) // TODO: spec
    }

    fn interpret_if(
        &mut self,
        cond: &Box<ExecExpression>,
        stats_true: &Vec<ExecStatement>,
        stats_false: &Vec<ExecStatement>,
    ) -> ExpressionFlow {
        let cond = try_expr!(self.interpret_expression(cond));
        match self.interpret_statements(if cond == 0 { stats_true } else { stats_false }) {
            Flow::Proceed => ExpressionFlow::Value(0),
            other => ExpressionFlow::Jump(other),
        }
    }

    fn interpret_operation1(
        &mut self,
        op: &Operator1,
        expr1: &Box<ExecExpression>,
    ) -> ExpressionFlow {
        let v1 = try_expr!(self.interpret_expression(expr1));
        let res = match op {
            Operator1::Negative => -v1,
        };
        ExpressionFlow::Value(res)
    }

    fn interpret_operation2(
        &mut self,
        op: &Operator2,
        expr1: &Box<ExecExpression>,
        expr2: &Box<ExecExpression>,
    ) -> ExpressionFlow {
        if let Operator2::Assign = op {
            if let ExecExpression::Variable(name) = expr1.as_ref() {
                if self.variables.contains_key(name) {
                    // todo: more nice impl
                    // todo: should be checked not in runtime.
                    let v = try_expr!(self.interpret_expression(expr2));
                    self.variables.insert(name.clone(), v);
                    return ExpressionFlow::Value(v);
                } else {
                    panic!("syntax error: unknown variable name `{}`", name)
                }
            } else {
                panic!("runtime error: left value is not variable");
            }
        }
        let v1 = try_expr!(self.interpret_expression(expr1));
        let v2 = try_expr!(self.interpret_expression(expr2));
        let res = match op {
            Operator2::Plus => v1 + v2,
            Operator2::Minus => v1 - v2,
            Operator2::Multiply => v1 * v2,
            Operator2::Divide => v1 / v2,
            Operator2::Assign => unreachable!(),
            Operator2::Equal => bool_to_int(v1 == v2),
            Operator2::NotEqual => bool_to_int(v1 != v2),
            Operator2::Less => bool_to_int(v1 < v2),
            Operator2::LessEqual => bool_to_int(v1 <= v2),
            Operator2::Greater => bool_to_int(v1 > v2),
            Operator2::GreaterEqual => bool_to_int(v1 >= v2),
        };
        ExpressionFlow::Value(res)
    }

    // if while を式にした以上、式の中に文が含まれる可能性がある…
    fn interpret_expression(&mut self, expr: &Box<ExecExpression>) -> ExpressionFlow {
        match expr.as_ref() {
            ExecExpression::Operation1(op, expr1) => self.interpret_operation1(op, expr1),
            ExecExpression::Operation2(op, expr1, expr2) => {
                self.interpret_operation2(op, expr1, expr2)
            }
            ExecExpression::Function(id, args) => self.interpret_call_function(id, args),
            ExecExpression::Factor(v) => ExpressionFlow::Value(*v),
            ExecExpression::Variable(name) => {
                if let Some(val) = self.variables.get(name) {
                    ExpressionFlow::Value(*val)
                } else {
                    panic!("syntax error: unknown variable name")
                }
            }
            ExecExpression::If(cond, stats_true, stats_false) => {
                self.interpret_if(cond, stats_true, stats_false)
            }
            ExecExpression::While(cond, code) => self.interpret_while(cond, code),
        }
    }

    fn interpret_statement(&mut self, statement: &ExecStatement) -> Flow {
        match statement {
            ExecStatement::Expression(expr) => match self.interpret_expression(expr) {
                ExpressionFlow::Value(_) => Flow::Proceed,
                ExpressionFlow::Jump(j) => j,
            },
            ExecStatement::Return(expr) => match self.interpret_expression(expr) {
                ExpressionFlow::Value(res) => Flow::Return(res),
                ExpressionFlow::Jump(j) => j,
            },
            ExecStatement::Break => Flow::Break,
            ExecStatement::Continue => Flow::Continue,
        }
    }

    pub fn interpret_statements(&mut self, statements: &Vec<ExecStatement>) -> Flow {
        for statement in statements {
            match self.interpret_statement(statement) {
                Flow::Proceed => (),
                other => return other,
            }
        }
        Flow::Proceed
    }
}

pub fn interpret_func(env: &mut Environment, scope: &Scope, func_name: &str) -> Option<i64> {
    let func = scope.get_function(func_name).unwrap();
    let mut e = LocalEnvironment::new_func(env, scope, &func, &Vec::<i64>::new());
    let res = e.interpret_statements(&func.code);
    if let Flow::Return(x) = res {
        Some(x)
    } else {
        None
    }
}
