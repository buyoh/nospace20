//! # Interpreter
//!
//! コンパイル前のコードを実行します。
//! コンパイラの実装は多様で複雑になりがちな為、Interpreterは極力シンプルな実装となるよう
//! 他のモジュールを設計しなければなりません。
//!

use std::collections::BTreeMap;
use std::io::{BufRead, Write};

use crate::{
    semantic_analyzer::{ExecExpression, ExecStatement, Function, Scope},
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

/// インタプリタの実行制限設定
pub struct EnvironmentConfig {
    /// Expression評価の最大実行回数 (Noneの場合は無制限)
    pub max_expression_count: Option<usize>,
}

impl EnvironmentConfig {
    pub fn new() -> Self {
        EnvironmentConfig {
            max_expression_count: None,
        }
    }

    pub fn with_max_expression_count(max_count: usize) -> Self {
        EnvironmentConfig {
            max_expression_count: Some(max_count),
        }
    }
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// インタプリタの実行メトリクス
pub struct EnvironmentMetrics {
    expression_count: usize,
}

impl EnvironmentMetrics {
    pub fn new() -> Self {
        EnvironmentMetrics {
            expression_count: 0,
        }
    }

    pub fn expression_count(&self) -> usize {
        self.expression_count
    }
}

impl Default for EnvironmentMetrics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Environment {
    pub traced: BTreeMap<i64, i64>,
    pub(crate) stdin: Box<dyn BufRead>,
    pub(crate) stdout: Box<dyn Write>,
    pub config: EnvironmentConfig,
    metrics: EnvironmentMetrics,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin: Box::new(std::io::BufReader::new(std::io::stdin())),
            stdout: Box::new(std::io::stdout()),
            config: EnvironmentConfig::new(),
            metrics: EnvironmentMetrics::new(),
        }
    }

    pub fn new_with_buffers(stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin,
            stdout,
            config: EnvironmentConfig::new(),
            metrics: EnvironmentMetrics::new(),
        }
    }

    pub fn new_with_config(
        stdin: Box<dyn BufRead>,
        stdout: Box<dyn Write>,
        config: EnvironmentConfig,
    ) -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin,
            stdout,
            config,
            metrics: EnvironmentMetrics::new(),
        }
    }

    fn increment_expression_count(&mut self) {
        self.metrics.expression_count += 1;
        if let Some(max) = self.config.max_expression_count {
            if self.metrics.expression_count > max {
                panic!(
                    "Expression evaluation limit exceeded: {} > {}",
                    self.metrics.expression_count, max
                );
            }
        }
    }

    pub fn metrics(&self) -> &EnvironmentMetrics {
        &self.metrics
    }

    pub fn write_int(&mut self, val: i64) {
        write!(self.stdout, "{}", val).unwrap();
    }

    pub fn write_char(&mut self, val: i64) {
        let byte = (val as u8) as char;
        write!(self.stdout, "{}", byte).unwrap();
    }

    pub fn flush(&mut self) {
        self.stdout.flush().unwrap();
    }

    pub fn read_int(&mut self) -> i64 {
        let mut buf = String::new();
        let mut chars_read = 0;
        let mut negative = false;
        let mut num_str = String::new();

        // 空白・改行をスキップして数値を読み取る
        loop {
            buf.clear();
            match self.stdin.read_line(&mut buf) {
                Ok(0) => return 0, // EOF
                Ok(_) => {
                    for ch in buf.chars() {
                        if chars_read == 0 && (ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t')
                        {
                            continue; // 先頭の空白をスキップ
                        }
                        if chars_read == 0 && ch == '-' {
                            negative = true;
                            chars_read += 1;
                            continue;
                        }
                        if ch.is_ascii_digit() {
                            num_str.push(ch);
                            chars_read += 1;
                        } else if chars_read > 0 {
                            // 数値の終わり
                            break;
                        }
                    }
                    if chars_read > 0 {
                        break;
                    }
                }
                Err(_) => return 0,
            }
        }

        let result = num_str.parse::<i64>().unwrap_or(0);
        if negative {
            -result
        } else {
            result
        }
    }

    pub fn read_char(&mut self) -> i64 {
        let mut buf = [0u8; 1];
        match self.stdin.read(&mut buf) {
            Ok(1) => buf[0] as i64,
            _ => 0, // EOF
        }
    }
}

// 1つのfunction scopeの`実行時インスタンス`を管理する
struct LocalEnvironment<'a, 'aenv> {
    env: &'aenv mut Environment,
    root_scope: &'a Scope,
    current_scope: &'a Scope,
    variables: BTreeMap<String, i64>,
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
        let mut variables = BTreeMap::<String, i64>::new();
        for id_eval in func.args.iter().zip(args) {
            variables.insert(id_eval.0.clone(), *id_eval.1);
        }
        for v in func.scope.variables.iter() {
            if !variables.contains_key(&v.identifier) {
                variables.insert(v.identifier.clone(), 0);
            }
        }
        LocalEnvironment {
            env,
            root_scope,
            current_scope: &func.scope,
            variables,
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
            "__puti" => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                self.env.write_int(a);
                ExpressionFlow::Value(a)
            }
            "__putc" => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                self.env.write_char(a);
                ExpressionFlow::Value(a)
            }
            "__geti" => {
                let val = self.env.read_int();
                ExpressionFlow::Value(val)
            }
            "__getc" => {
                let val = self.env.read_char();
                ExpressionFlow::Value(val)
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
            Flow::Return(x) => ExpressionFlow::Value(x), // 関数の return は呼び出し元の式の値となる
            Flow::Continue => panic!("internal error: unexpected continue"),
            Flow::Break => panic!("internal error: unexpected break"),
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
        match self.interpret_statements(if cond != 0 { stats_true } else { stats_false }) {
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
            Operator1::LogicalNot => bool_to_int(v1 == 0),
        };
        ExpressionFlow::Value(res)
    }

    fn interpret_operation2(
        &mut self,
        op: &Operator2,
        expr1: &Box<ExecExpression>,
        expr2: &Box<ExecExpression>,
    ) -> ExpressionFlow {
        // 代入演算子: 特別処理
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
        // 論理AND: 短絡評価 (左辺が0なら右辺を評価せず0を返す)
        if let Operator2::LogicalAnd = op {
            let v1 = try_expr!(self.interpret_expression(expr1));
            if v1 == 0 {
                return ExpressionFlow::Value(0);
            }
            let v2 = try_expr!(self.interpret_expression(expr2));
            return ExpressionFlow::Value(bool_to_int(v2 != 0));
        }
        // 論理OR: 短絡評価 (左辺が非0なら右辺を評価せず1を返す)
        if let Operator2::LogicalOr = op {
            let v1 = try_expr!(self.interpret_expression(expr1));
            if v1 != 0 {
                return ExpressionFlow::Value(1);
            }
            let v2 = try_expr!(self.interpret_expression(expr2));
            return ExpressionFlow::Value(bool_to_int(v2 != 0));
        }
        let v1 = try_expr!(self.interpret_expression(expr1));
        let v2 = try_expr!(self.interpret_expression(expr2));
        let res = match op {
            Operator2::Plus => v1 + v2,
            Operator2::Minus => v1 - v2,
            Operator2::Multiply => v1 * v2,
            Operator2::Divide => v1 / v2,
            Operator2::Modulo => v1 % v2,
            Operator2::Assign => unreachable!(),
            Operator2::Equal => bool_to_int(v1 == v2),
            Operator2::NotEqual => bool_to_int(v1 != v2),
            Operator2::Less => bool_to_int(v1 < v2),
            Operator2::LessEqual => bool_to_int(v1 <= v2),
            Operator2::Greater => bool_to_int(v1 > v2),
            Operator2::GreaterEqual => bool_to_int(v1 >= v2),
            Operator2::LogicalAnd => unreachable!(),
            Operator2::LogicalOr => unreachable!(),
        };
        ExpressionFlow::Value(res)
    }

    // if while を式にした以上、式の中に文が含まれる可能性がある…
    fn interpret_expression(&mut self, expr: &Box<ExecExpression>) -> ExpressionFlow {
        self.env.increment_expression_count();
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
