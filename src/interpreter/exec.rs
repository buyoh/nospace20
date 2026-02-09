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

    /// IdentifierRef から絶対アドレスを計算（Phase 3）
    fn resolve_address(&self, id: &IdentifierRef) -> i64 {
        if id.is_global {
            id.local_index as i64
        } else {
            let global_count = self.env.global_variables.len() as i64;
            let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
            let mut addr = global_count;
            for i in 0..scope_idx {
                addr += self.scope_stack[i].len() as i64;
            }
            addr + id.local_index as i64
        }
    }

    /// 絶対アドレスから値を取得（Phase 3）
    fn get_by_address(&self, addr: i64) -> i64 {
        let addr = addr as usize;
        let global_count = self.env.global_variables.len();
        if addr < global_count {
            self.env.global_variables[addr]
        } else {
            let mut remaining = addr - global_count;
            for scope in &self.scope_stack {
                if remaining < scope.len() {
                    return scope[remaining];
                }
                remaining -= scope.len();
            }
            panic!("runtime error: invalid address {}", addr);
        }
    }

    /// 絶対アドレスに値を設定（Phase 3）
    fn set_by_address(&mut self, addr: i64, value: i64) {
        let addr = addr as usize;
        let global_count = self.env.global_variables.len();
        if addr < global_count {
            self.env.global_variables[addr] = value;
        } else {
            let mut remaining = addr - global_count;
            for scope in &mut self.scope_stack {
                if remaining < scope.len() {
                    scope[remaining] = value;
                    return;
                }
                remaining -= scope.len();
            }
            panic!("runtime error: invalid address {}", addr);
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
                if !self.env.config.ignore_debug {
                    println!("__clog: {}", a);
                }
                ExpressionFlow::Value(a)
            }
            "__assert" => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                if !self.env.config.ignore_debug && a == 0 {
                    // TODO: 気の利いたログを出せない
                    panic!("assertion failed: {} == 0", a);
                }
                ExpressionFlow::Value(a)
            }
            "__assert_not" => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                if !self.env.config.ignore_debug && a != 0 {
                    // TODO: 気の利いたログを出せない
                    panic!("assertion failed: {} != 0", a);
                }
                ExpressionFlow::Value(a)
            }
            "__trace" => {
                // TODO: 未だ比較演算子を実装していないので not
                let key = try_expr!(self.interpret_expression(args.first().unwrap()));
                if !self.env.config.ignore_debug {
                    let traced = &mut self.env.traced;
                    if let Some(v) = traced.get_mut(&key) {
                        *v += 1;
                    } else {
                        traced.insert(key, 1);
                    }
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

        // 新しい scope を既存の scope_stack に push
        let mut variables = vec![0; func.block.scope.variable_count];
        for (i, arg_val) in arg_values.iter().enumerate() {
            if i < func.arg_indices.len() {
                variables[func.arg_indices[i]] = *arg_val;
            }
        }
        self.scope_stack.push(variables);

        // 既存の LocalEnvironment 上で関数本体を実行
        let result = match self.interpret_statements(&func.block.statements) {
            Flow::Proceed => ExpressionFlow::Value(0),
            Flow::Return(x) => ExpressionFlow::Value(x),
            Flow::Continue => panic!("internal error: unexpected continue"),
            Flow::Break => panic!("internal error: unexpected break"),
        };

        // 関数スコープを pop
        self.scope_stack.pop();
        result
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
        match op {
            Operator1::Ref => {
                match expr1.as_ref() {
                    ExecExpression::Variable(id_ref) => {
                        let addr = self.resolve_address(id_ref);
                        ExpressionFlow::Value(addr)
                    }
                    ExecExpression::ArrayAccess(id_ref, index_expr, array_size) => {
                        let index = try_expr!(self.interpret_expression(index_expr));

                        // 境界チェック
                        if index < 0 || index >= *array_size as i64 {
                            panic!(
                                "runtime error: array index out of bounds: index {} but size {}",
                                index, array_size
                            );
                        }

                        let base_addr = self.resolve_address(id_ref);
                        ExpressionFlow::Value(base_addr + index)
                    }
                    _ => {
                        panic!("runtime error: cannot take reference of non-variable");
                    }
                }
            }
            Operator1::Deref => {
                let addr = try_expr!(self.interpret_expression(expr1));
                let value = self.get_by_address(addr);
                ExpressionFlow::Value(value)
            }
            _ => {
                let v1 = try_expr!(self.interpret_expression(expr1));
                let res = match op {
                    Operator1::Negative => -v1,
                    Operator1::LogicalNot => bool_to_int(v1 == 0),
                    _ => unreachable!(),
                };
                ExpressionFlow::Value(res)
            }
        }
    }

    fn interpret_operation2(
        &mut self,
        op: &Operator2,
        expr1: &Box<ExecExpression>,
        expr2: &Box<ExecExpression>,
    ) -> ExpressionFlow {
        // 代入演算子: 特別処理
        if let Operator2::Assign = op {
            match expr1.as_ref() {
                ExecExpression::Variable(id_ref) => {
                    let v = try_expr!(self.interpret_expression(expr2));
                    // Phase 2: IdentifierRef を使用して O(1) でアクセス
                    self.set_variable(id_ref, v);
                    return ExpressionFlow::Value(v);
                }
                ExecExpression::ArrayAccess(id_ref, index_expr, array_size) => {
                    // 配列要素への代入: arr[i] = val
                    let index = try_expr!(self.interpret_expression(index_expr));
                    let v = try_expr!(self.interpret_expression(expr2));

                    // 境界チェック
                    if index < 0 || index >= *array_size as i64 {
                        panic!(
                            "runtime error: array index out of bounds: index {} but size {}",
                            index, array_size
                        );
                    }

                    let mut adjusted_ref = *id_ref;
                    adjusted_ref.local_index += index as usize;
                    self.set_variable(&adjusted_ref, v);
                    return ExpressionFlow::Value(v);
                }
                ExecExpression::Operation1(Operator1::Deref, inner) => {
                    // *ptr = value のケース (Phase 3)
                    let addr = try_expr!(self.interpret_expression(inner));
                    let v = try_expr!(self.interpret_expression(expr2));
                    self.set_by_address(addr, v);
                    return ExpressionFlow::Value(v);
                }
                _ => {
                    panic!("runtime error: left value is not assignable");
                }
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
            Operator2::PlusAssign => unreachable!(),
            Operator2::MinusAssign => unreachable!(),
            Operator2::MultiplyAssign => unreachable!(),
            Operator2::DivideAssign => unreachable!(),
            Operator2::ModuloAssign => unreachable!(),
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
            ExecExpression::ArrayAccess(id_ref, index_expr, array_size) => {
                let index = try_expr!(self.interpret_expression(index_expr));

                // 境界チェック
                if index < 0 || index >= *array_size as i64 {
                    panic!(
                        "runtime error: array index out of bounds: index {} but size {}",
                        index, array_size
                    );
                }

                // ベースアドレス + オフセット でアクセス
                let mut adjusted_ref = *id_ref;
                adjusted_ref.local_index += index as usize;
                ExpressionFlow::Value(self.get_variable(&adjusted_ref))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_to_tokens;
    use crate::parse_to_tree;
    use crate::semantic_analyzer::analyze;
    use crate::EnvironmentConfig;
    use std::io::Cursor;

    fn create_test_env() -> Environment {
        let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(Vec::<u8>::new())));
        let stdout_buf: Box<dyn std::io::Write> = Box::new(Vec::<u8>::new());
        Environment::new_with_config(stdin_cursor, stdout_buf, EnvironmentConfig::new())
    }

    fn parse_and_analyze(code: &str) -> Scope {
        let code_string = code.to_string();
        let tokens = parse_to_tokens(&code_string).expect("Failed to parse tokens");
        let tree = parse_to_tree(&tokens).expect("Failed to parse tree");
        analyze(&tree).expect("Failed to analyze")
    }

    #[test]
    fn test_resolve_address_local_variables() {
        let code = r#"
func: main() {
    let: x; let: p;
    x = 42;
    p = 0;
    return: 0;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        env.global_variables = vec![0; scope.variable_count];

        let func = scope.get_function("main").unwrap();
        let local_env = LocalEnvironment::new_func(&mut env, &scope, &func, &vec![]);

        // main 関数のローカル変数 x (local_index=0), p (local_index=1)
        let id_x = IdentifierRef {
            is_global: false,
            scope_depth: 0,
            local_index: 0,
        };
        let addr_x = local_env.resolve_address(&id_x);
        assert_eq!(addr_x, 0, "x should be at address 0");

        let id_p = IdentifierRef {
            is_global: false,
            scope_depth: 0,
            local_index: 1,
        };
        let addr_p = local_env.resolve_address(&id_p);
        assert_eq!(addr_p, 1, "p should be at address 1");
    }

    #[test]
    fn test_get_set_by_address() {
        let code = r#"
func: main() {
    let: x; let: p;
    return: 0;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        env.global_variables = vec![0; scope.variable_count];

        let func = scope.get_function("main").unwrap();
        let mut local_env = LocalEnvironment::new_func(&mut env, &scope, &func, &vec![]);

        // アドレス 0 に値を設定
        local_env.set_by_address(0, 42);
        let val = local_env.get_by_address(0);
        assert_eq!(
            val, 42,
            "get_by_address should return the value set by set_by_address"
        );

        // アドレス 1 に値を設定
        local_env.set_by_address(1, 99);
        let val = local_env.get_by_address(1);
        assert_eq!(val, 99, "get_by_address should return 99");
    }

    #[test]
    fn test_ref_and_deref_integration() {
        let code = r#"
func: main() {
    let: x; let: p;
    x = 42;
    p = &x;
    return: *p;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();

        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(
            result,
            Some(42),
            "should return the value of *p which is 42"
        );
    }

    #[test]
    fn test_deref_assign_integration() {
        let code = r#"
func: main() {
    let: x; let: p;
    x = 10;
    p = &x;
    *p = 20;
    return: x;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();

        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(20), "x should be modified to 20 via *p = 20");
    }
}
