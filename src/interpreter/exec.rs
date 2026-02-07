use crate::{
    semantic_analyzer::{Block, ExecExpression, ExecStatement, Function, IdentifierRef, Scope},
    tree_parser::{Operator1, Operator2},
};

use super::environment::Environment;
use super::types::{bool_to_int, try_expr, ExpressionFlow, Flow};

/// 1つのfunction scopeの`実行時インスタンス`を管理する
///
/// Phase 2 で scope_stack を BTreeMap<String, i64> から Vec<i64> に変更。
/// 変数アクセスを O(1) にするため、IdentifierRef を使用してインデックスベースでアクセスする。
pub(super) struct LocalEnvironment<'a, 'aenv> {
    pub(super) env: &'aenv mut Environment,
    pub(super) root_scope: &'a Scope,
    /// スコープスタック: 末尾が現在のスコープ
    /// Phase 2: BTreeMap から Vec<i64> に変更
    pub(super) scope_stack: Vec<Vec<i64>>,
}

impl LocalEnvironment<'_, '_> {
    pub(super) fn new_func<'a, 'aenv>(
        env: &'aenv mut Environment,
        root_scope: &'a Scope,
        func: &'a Function,
        args: &Vec<i64>,
    ) -> LocalEnvironment<'a, 'aenv> {
        // Phase 2: Vec<i64> ベースの変数管理
        // 変数の数だけ領域を確保し、引数で初期化
        let mut variables = vec![0; func.block.scope.variable_count];

        // 引数を対応する変数にセット（Phase 2 最適化: 事前計算されたインデックスを使用）
        for (i, arg_val) in args.iter().enumerate() {
            if i < func.arg_indices.len() {
                // 事前計算されたインデックスを使用して O(1) でアクセス
                variables[func.arg_indices[i]] = *arg_val;
            }
        }

        LocalEnvironment {
            env,
            root_scope,
            scope_stack: vec![variables],
        }
    }

    /// ブロックに入る
    fn enter_block(&mut self, scope: &Scope) {
        // Phase 2: 変数の数だけ Vec を初期化
        self.scope_stack.push(vec![0; scope.variable_count]);
    }

    /// ブロックから出る
    fn leave_block(&mut self) {
        self.scope_stack.pop();
    }

    /// 識別子参照から値を取得（Phase 2）
    /// Phase 3: グローバル変数対応（is_global フラグチェック）
    fn get_variable(&self, id: &IdentifierRef) -> i64 {
        if id.is_global {
            // グローバル変数は Environment に保持
            self.env.global_variables[id.local_index]
        } else {
            // ローカル変数は scope_stack に保持
            let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
            self.scope_stack[scope_idx][id.local_index]
        }
    }

    /// 識別子参照に値を設定（Phase 2）
    /// Phase 3: グローバル変数対応（is_global フラグチェック）
    fn set_variable(&mut self, id: &IdentifierRef, value: i64) {
        if id.is_global {
            // グローバル変数は Environment に保持
            self.env.global_variables[id.local_index] = value;
        } else {
            // ローカル変数は scope_stack に保持
            let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
            self.scope_stack[scope_idx][id.local_index] = value;
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
        match env.interpret_statements(&func.block.statements) {
            Flow::Proceed => ExpressionFlow::Value(0),
            Flow::Return(x) => ExpressionFlow::Value(x), // 関数の return は呼び出し元の式の値となる
            Flow::Continue => panic!("internal error: unexpected continue"),
            Flow::Break => panic!("internal error: unexpected break"),
        }
    }

    fn interpret_while(&mut self, cond: &Box<ExecExpression>, block: &Block) -> ExpressionFlow {
        let mut last_value = 0;
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
            self.enter_block(&block.scope);
            let (flow, value) = self.interpret_statements_with_value(&block.statements);
            let result = match flow {
                Flow::Proceed => {
                    last_value = value;
                    None
                }
                Flow::Return(v) => Some(ExpressionFlow::Jump(Flow::Return(v))),
                Flow::Continue => {
                    last_value = value;
                    None
                }
                Flow::Break => {
                    // break で抜けた場合は 0 を返す仕様とする
                    last_value = 0;
                    self.leave_block();
                    break;
                }
            };
            self.leave_block();
            if let Some(r) = result {
                return r;
            }
        }
        ExpressionFlow::Value(last_value)
    }

    fn interpret_if(
        &mut self,
        cond: &Box<ExecExpression>,
        then_block: &Block,
        else_block: &Block,
    ) -> ExpressionFlow {
        let cond = try_expr!(self.interpret_expression(cond));
        let block = if cond != 0 { then_block } else { else_block };
        self.enter_block(&block.scope);
        let (flow, value) = self.interpret_statements_with_value(&block.statements);
        let result = match flow {
            Flow::Proceed => ExpressionFlow::Value(value),
            other => ExpressionFlow::Jump(other),
        };
        self.leave_block();
        result
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
            if let ExecExpression::Variable(id_ref) = expr1.as_ref() {
                let v = try_expr!(self.interpret_expression(expr2));
                // Phase 2: IdentifierRef を使用して O(1) でアクセス
                self.set_variable(id_ref, v);
                return ExpressionFlow::Value(v);
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
            ExecExpression::Variable(id_ref) => {
                // Phase 2: IdentifierRef を使用して O(1) でアクセス
                ExpressionFlow::Value(self.get_variable(id_ref))
            }
            ExecExpression::If(cond, then_block, else_block) => {
                self.interpret_if(cond, then_block, else_block)
            }
            ExecExpression::While(cond, block) => self.interpret_while(cond, block),
        }
    }

    /// ブロックの文を実行し、最後の式の値も返す
    /// if/while 式の戻り値を実装するために使用
    fn interpret_statements_with_value(&mut self, statements: &Vec<ExecStatement>) -> (Flow, i64) {
        let mut last_value = 0;
        for statement in statements {
            match statement {
                ExecStatement::Expression(expr) => match self.interpret_expression(expr) {
                    ExpressionFlow::Value(v) => last_value = v,
                    ExpressionFlow::Jump(j) => return (j, last_value),
                },
                ExecStatement::Return(expr) => match self.interpret_expression(expr) {
                    ExpressionFlow::Value(res) => return (Flow::Return(res), res),
                    ExpressionFlow::Jump(j) => return (j, last_value),
                },
                ExecStatement::Break => return (Flow::Break, last_value),
                ExecStatement::Continue => return (Flow::Continue, last_value),
            }
        }
        (Flow::Proceed, last_value)
    }

    pub(super) fn interpret_statements(&mut self, statements: &Vec<ExecStatement>) -> Flow {
        let (flow, _) = self.interpret_statements_with_value(statements);
        flow
    }

    pub(super) fn interpret_statement(&mut self, statement: &ExecStatement) -> Flow {
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
}
