use crate::{
    semantic_analyzer::{
        Block, ConditionMode, ExecExpression, ExecStatement, Function, IdentifierRef,
        InternalBuiltinFunctionKind, LocatedExecExpression, LocatedExecStatement, Scope,
    },
    tree_parser::{Operator1, Operator2},
};

use super::environment::Environment;
use super::types::{bool_to_int, try_expr, ExpressionFlow, Flow};

use std::cell::RefCell;

thread_local! {
    static UNINIT_COUNTER: RefCell<u64> = RefCell::new(0);
}

/// 未初期化変数のフィル値を生成する（決定論的な非自明値）
///
/// 0 でない値を返すことで、初期値 0 への暗黙依存バグを検出しやすくする。
/// スレッドローカルなカウンタを使い、外部 crate なしで生成する。
pub(super) fn random_uninit_value() -> i64 {
    UNINIT_COUNTER.with(|c| {
        let mut count = c.borrow_mut();
        *count = count.wrapping_add(1);
        // 簡易ハッシュ: 0 を避けつつ決定論的な非自明値を生成
        let v = (*count)
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        v as i64
    })
}

/// 変数領域の初期フィル値を返す
///
/// `randomize` が true のときランダム値、false のとき 0 を返す。
fn uninit_fill_value(randomize: bool) -> i64 {
    if randomize {
        random_uninit_value()
    } else {
        0
    }
}

/// 1つのfunction scopeの`実行時インスタンス`を管理する
///
/// scope_stack を BTreeMap<String, i64> から Vec<i64> に変更。
/// 変数アクセスを O(1) にするため、IdentifierRef を使用してインデックスベースでアクセスする。
pub(super) struct LocalEnvironment<'a, 'aenv> {
    pub(super) env: &'aenv mut Environment,
    pub(super) root_scope: &'a Scope,
    /// スコープスタック: 末尾が現在のスコープ
    pub(super) scope_stack: Vec<Vec<i64>>,
}

impl LocalEnvironment<'_, '_> {
    pub(super) fn new_func<'a, 'aenv>(
        env: &'aenv mut Environment,
        root_scope: &'a Scope,
        func: &'a Function,
        args: &Vec<i64>,
    ) -> LocalEnvironment<'a, 'aenv> {
        // Vec<i64> ベースの変数管理
        // 変数の数だけ領域を確保し、引数で初期化
        let randomize = env.config.randomize_uninit;
        let mut variables: Vec<i64> = (0..func.block.scope.variable_count)
            .map(|_| uninit_fill_value(randomize))
            .collect();

        // 引数を対応する変数にセット（最適化: 事前計算されたインデックスを使用）
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
        // 変数の数だけ Vec を初期化（randomize_uninit モードではランダム値で埋める）
        let randomize = self.env.config.randomize_uninit;
        let vars: Vec<i64> = (0..scope.variable_count)
            .map(|_| uninit_fill_value(randomize))
            .collect();
        self.scope_stack.push(vars);
    }

    /// ブロックから出る
    fn leave_block(&mut self) {
        self.scope_stack.pop();
    }

    /// 識別子参照から値を取得
    /// グローバル変数対応（is_global フラグチェック）
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

    /// 識別子参照に値を設定
    /// グローバル変数対応（is_global フラグチェック）
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

    /// IdentifierRef から絶対アドレスを計算
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

    /// 絶対アドレスから値を取得
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

    /// 絶対アドレスに値を設定
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
        kind: &crate::semantic_analyzer::BuiltinFunctionKind,
        args: &Vec<Box<LocatedExecExpression>>,
    ) -> ExpressionFlow {
        use crate::semantic_analyzer::BuiltinFunctionKind;

        match kind {
            BuiltinFunctionKind::Clog => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                if !self.env.config.ignore_debug {
                    println!("__clog: {}", a);
                }
                ExpressionFlow::Value(a)
            }
            BuiltinFunctionKind::Assert => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                if !self.env.config.ignore_debug && a == 0 {
                    panic!("assertion failed: {} == 0", a);
                }
                ExpressionFlow::Value(a)
            }
            BuiltinFunctionKind::AssertNot => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                if !self.env.config.ignore_debug && a != 0 {
                    panic!("assertion failed: {} != 0", a);
                }
                ExpressionFlow::Value(a)
            }
            BuiltinFunctionKind::Trace => {
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
            BuiltinFunctionKind::Puti => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                self.env.write_int(a);
                ExpressionFlow::Value(a)
            }
            BuiltinFunctionKind::Putc => {
                let a = try_expr!(self.interpret_expression(args.first().unwrap()));
                self.env.write_char(a);
                ExpressionFlow::Value(a)
            }
            BuiltinFunctionKind::Geti => {
                let val = self.env.read_int();
                ExpressionFlow::Value(val)
            }
            BuiltinFunctionKind::Getc => {
                let val = self.env.read_char();
                ExpressionFlow::Value(val)
            }
            BuiltinFunctionKind::Alloc | BuiltinFunctionKind::Free => {
                panic!(
                    "runtime error: __alloc/__free are not supported in interpreter mode. \
                     Use --mode=compile --std=ws --std-ext alloc instead."
                );
            }
        }
    }

    /// Phase 5: IdentifierRef を使用してユーザー定義関数を呼び出す
    fn interpret_call_user_function_by_ref(
        &mut self,
        func_ref: &IdentifierRef,
        args: &Vec<Box<LocatedExecExpression>>,
    ) -> ExpressionFlow {
        let mut arg_values = Vec::new();
        arg_values.reserve(args.len());
        for a in args {
            arg_values.push(try_expr!(self.interpret_expression(a)));
        }

        // IdentifierRef から関数を取得
        // Phase 5: 全関数は root_scope にフラット化されているため、
        // 常に root_scope.functions から取得する
        let func = &self.root_scope.functions[func_ref.local_index];

        // Phase 4: static 変数の永続化対応
        let has_static = func.block.scope.variables.iter().any(|v| v.is_static);

        // 新しい scope を既存の scope_stack に push（randomize_uninit モードではランダム値で初期化）
        let randomize = self.env.config.randomize_uninit;
        let mut variables: Vec<i64> = (0..func.block.scope.variable_count)
            .map(|_| uninit_fill_value(randomize))
            .collect();

        // static 変数があり、永続ストレージが存在する場合は値を復元
        // 関数インデックスをキーとして使用
        let func_key = func_ref.local_index;
        if has_static {
            if let Some(storage) = self.env.function_static_storage.get(&func_key) {
                for var in &func.block.scope.variables {
                    if var.is_static {
                        let slot_idx = var.slot_index;
                        let slot_count = var.array_size.unwrap_or(1);
                        for i in 0..slot_count {
                            variables[slot_idx + i] = storage[slot_idx + i];
                        }
                    }
                }
            }
        }

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

        // Phase 4: static 変数の値を永続ストレージに保存
        if has_static {
            let scope_data = self.scope_stack.last().unwrap().clone();
            self.env
                .function_static_storage
                .insert(func_key, scope_data);
        }

        // 関数スコープを pop
        self.scope_stack.pop();
        result
    }

    /// while 文のループ実行
    fn interpret_while_statement(
        &mut self,
        mode: &ConditionMode,
        cond: &Box<LocatedExecExpression>,
        block: &Block,
    ) -> Flow {
        loop {
            let cond_val = match self.interpret_expression(cond) {
                ExpressionFlow::Value(e) => e,
                ExpressionFlow::Jump(Flow::Return(x)) => return Flow::Return(x),
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
            let condition = match mode {
                ConditionMode::NonZero => cond_val != 0,
                ConditionMode::Zero => cond_val == 0,
                ConditionMode::Negative => cond_val < 0,
            };
            if !condition {
                break;
            }
            self.enter_block(&block.scope);
            let (flow, _value) = self.interpret_statements_with_value(&block.statements);
            match flow {
                Flow::Proceed | Flow::Continue => {
                    self.leave_block();
                }
                Flow::Return(v) => {
                    self.leave_block();
                    return Flow::Return(v);
                }
                Flow::Break => {
                    self.leave_block();
                    break;
                }
            }
        }
        Flow::Proceed
    }

    /// for 文のループ実行
    ///
    /// continue は step ブロックを実行してから条件評価に戻る（while とは異なる）。
    fn interpret_for_statement(
        &mut self,
        init: &Block,
        mode: &ConditionMode,
        cond: &Block,
        step: &Block,
        body: &Block,
    ) -> Flow {
        // for スコープ（init 変数のスコープ）に入る
        self.enter_block(&init.scope);

        // 初期化ブロックを実行
        let (init_flow, _) = self.interpret_statements_with_value(&init.statements);
        match init_flow {
            Flow::Return(v) => {
                self.leave_block();
                return Flow::Return(v);
            }
            Flow::Break | Flow::Continue => {
                // 初期化ブロックで break/continue は意味をなさないが、安全にハンドリング
                self.leave_block();
                return Flow::Proceed;
            }
            Flow::Proceed => {}
        }

        loop {
            // 条件ブロックを評価
            self.enter_block(&cond.scope);
            let (cond_flow, cond_val) = self.interpret_statements_with_value(&cond.statements);
            self.leave_block();
            match cond_flow {
                Flow::Return(v) => {
                    self.leave_block(); // init scope
                    return Flow::Return(v);
                }
                Flow::Break => {
                    self.leave_block(); // init scope
                    return Flow::Proceed;
                }
                Flow::Continue | Flow::Proceed => {}
            }

            // 条件判定
            let condition = match mode {
                ConditionMode::NonZero => cond_val != 0,
                ConditionMode::Zero => cond_val == 0,
                ConditionMode::Negative => cond_val < 0,
            };
            if !condition {
                break;
            }

            // body ブロックを実行
            self.enter_block(&body.scope);
            let (body_flow, _) = self.interpret_statements_with_value(&body.statements);
            self.leave_block();
            match body_flow {
                Flow::Return(v) => {
                    self.leave_block(); // init scope
                    return Flow::Return(v);
                }
                Flow::Break => {
                    break;
                }
                Flow::Proceed | Flow::Continue => {
                    // continue の場合は step を実行してから条件再評価
                }
            }

            // step ブロックを実行
            self.enter_block(&step.scope);
            let (step_flow, _) = self.interpret_statements_with_value(&step.statements);
            self.leave_block();
            match step_flow {
                Flow::Return(v) => {
                    self.leave_block(); // init scope
                    return Flow::Return(v);
                }
                Flow::Break => {
                    break;
                }
                Flow::Continue | Flow::Proceed => {}
            }
        }

        // for スコープ（init 変数スコープ）を出る
        self.leave_block();
        Flow::Proceed
    }

    fn interpret_if(
        &mut self,
        mode: &ConditionMode,
        cond: &Box<LocatedExecExpression>,
        then_block: &Block,
        else_block: &Block,
    ) -> ExpressionFlow {
        let cond_val = try_expr!(self.interpret_expression(cond));
        let condition = match mode {
            ConditionMode::NonZero => cond_val != 0,
            ConditionMode::Zero => cond_val == 0,
            ConditionMode::Negative => cond_val < 0,
        };
        let block = if condition { then_block } else { else_block };
        self.enter_block(&block.scope);
        let (flow, value) = self.interpret_statements_with_value(&block.statements);
        let result = match flow {
            Flow::Proceed => ExpressionFlow::Value(value),
            other => ExpressionFlow::Jump(other),
        };
        self.leave_block();
        result
    }

    fn interpret_block(&mut self, block: &Block) -> ExpressionFlow {
        self.enter_block(&block.scope);
        let (flow, value) = self.interpret_statements_with_value(&block.statements);
        let result = match flow {
            Flow::Proceed => ExpressionFlow::Value(value),
            other => ExpressionFlow::Jump(other),
        };
        self.leave_block();
        result
    }

    /// 最適化パスで生成される内部組み込み関数を実行する
    fn interpret_internal_builtin_function(
        &mut self,
        kind: &InternalBuiltinFunctionKind,
    ) -> ExpressionFlow {
        match kind {
            InternalBuiltinFunctionKind::Getiv(var_ref) => {
                let value = self.env.read_int();
                self.set_variable(var_ref, value);
                ExpressionFlow::Value(value)
            }
            InternalBuiltinFunctionKind::Getcv(var_ref) => {
                let value = self.env.read_char();
                self.set_variable(var_ref, value);
                ExpressionFlow::Value(value)
            }
        }
    }

    fn interpret_operation1(
        &mut self,
        op: &Operator1,
        expr1: &Box<LocatedExecExpression>,
    ) -> ExpressionFlow {
        match op {
            Operator1::Ref => {
                match &expr1.expression {
                    ExecExpression::Variable(id_ref) => {
                        let addr = self.resolve_address(id_ref);
                        ExpressionFlow::Value(addr)
                    }
                    ExecExpression::ArrayAccess(id_ref, index_expr, _array_size) => {
                        let index = try_expr!(self.interpret_expression(index_expr));

                        // p[i] は *(&p + i) と同義なので境界チェックは行わない
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
        expr1: &Box<LocatedExecExpression>,
        expr2: &Box<LocatedExecExpression>,
    ) -> ExpressionFlow {
        // 代入演算子: 特別処理
        if let Operator2::Assign = op {
            match &expr1.expression {
                ExecExpression::Variable(id_ref) => {
                    let v = try_expr!(self.interpret_expression(expr2));
                    // Phase 2: IdentifierRef を使用して O(1) でアクセス
                    self.set_variable(id_ref, v);
                    return ExpressionFlow::Value(v);
                }
                ExecExpression::ArrayAccess(id_ref, index_expr, _array_size) => {
                    // 配列要素への代入: arr[i] = val
                    // p[i] は *(&p + i) と同義なので境界チェックは行わない
                    let index = try_expr!(self.interpret_expression(index_expr));
                    let v = try_expr!(self.interpret_expression(expr2));

                    let mut adjusted_ref = *id_ref;
                    adjusted_ref.local_index = (adjusted_ref.local_index as i64 + index) as usize;
                    self.set_variable(&adjusted_ref, v);
                    return ExpressionFlow::Value(v);
                }
                ExecExpression::Operation1(Operator1::Deref, inner) => {
                    // *ptr = value のケース
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
    fn interpret_expression(
        &mut self,
        located_expr: &Box<LocatedExecExpression>,
    ) -> ExpressionFlow {
        self.env.increment_expression_count();
        match &located_expr.expression {
            ExecExpression::Operation1(op, expr1) => self.interpret_operation1(op, expr1),
            ExecExpression::Operation2(op, expr1, expr2) => {
                self.interpret_operation2(op, expr1, expr2)
            }
            // Phase 5: BuiltinFunction と UserFunction に分離
            // Phase 6: BuiltinFunction は BuiltinFunctionKind enum を使用
            ExecExpression::BuiltinFunction(kind, args) => self.interpret_call_function(kind, args),
            ExecExpression::UserFunction(func_ref, args) => {
                self.interpret_call_user_function_by_ref(func_ref, args)
            }
            ExecExpression::Factor(v) => ExpressionFlow::Value(*v),
            ExecExpression::Variable(id_ref) => {
                // IdentifierRef を使用して O(1) でアクセス
                ExpressionFlow::Value(self.get_variable(id_ref))
            }
            ExecExpression::ArrayAccess(id_ref, index_expr, _array_size) => {
                let index = try_expr!(self.interpret_expression(index_expr));

                // ベースアドレス + オフセット でアクセス
                // p[i] は *(&p + i) と同義なので境界チェックは行わない
                let mut adjusted_ref = *id_ref;
                adjusted_ref.local_index = (adjusted_ref.local_index as i64 + index) as usize;
                ExpressionFlow::Value(self.get_variable(&adjusted_ref))
            }
            ExecExpression::If(mode, cond, then_block, else_block) => {
                self.interpret_if(mode, cond, then_block, else_block)
            }
            ExecExpression::Block(block) => self.interpret_block(block),
            ExecExpression::InternalBuiltinFunction(kind) => {
                self.interpret_internal_builtin_function(kind)
            }
        }
    }

    /// ブロックの文を実行し、最後の式の値も返す
    /// if/while 式の戻り値を実装するために使用
    fn interpret_statements_with_value(
        &mut self,
        statements: &Vec<LocatedExecStatement>,
    ) -> (Flow, i64) {
        let mut last_value = 0;
        for located_stmt in statements {
            let statement = &located_stmt.statement;
            match statement {
                ExecStatement::Expression(expr) => match self.interpret_expression(expr) {
                    ExpressionFlow::Value(v) => last_value = v,
                    ExpressionFlow::Jump(j) => return (j, last_value),
                },
                ExecStatement::Return(Some(expr)) => match self.interpret_expression(expr) {
                    ExpressionFlow::Value(res) => return (Flow::Return(res), res),
                    ExpressionFlow::Jump(j) => return (j, last_value),
                },
                ExecStatement::Return(None) => return (Flow::Return(0), last_value),
                ExecStatement::Break => return (Flow::Break, last_value),
                ExecStatement::Continue => return (Flow::Continue, last_value),
                ExecStatement::While(mode, cond, block) => {
                    let flow = self.interpret_while_statement(mode, cond, block);
                    match flow {
                        Flow::Proceed => {}
                        other => return (other, last_value),
                    }
                }
                ExecStatement::For(init, mode, cond, step, body) => {
                    let flow = self.interpret_for_statement(init, mode, cond, step, body);
                    match flow {
                        Flow::Proceed => {}
                        other => return (other, last_value),
                    }
                }
            }
        }
        (Flow::Proceed, last_value)
    }

    pub(super) fn interpret_statements(&mut self, statements: &Vec<LocatedExecStatement>) -> Flow {
        let (flow, _) = self.interpret_statements_with_value(statements);
        flow
    }

    pub(super) fn interpret_statement(&mut self, statement: &ExecStatement) -> Flow {
        match statement {
            ExecStatement::Expression(expr) => match self.interpret_expression(expr) {
                ExpressionFlow::Value(_) => Flow::Proceed,
                ExpressionFlow::Jump(j) => j,
            },
            ExecStatement::Return(Some(expr)) => match self.interpret_expression(expr) {
                ExpressionFlow::Value(res) => Flow::Return(res),
                ExpressionFlow::Jump(j) => j,
            },
            ExecStatement::Return(None) => Flow::Return(0),
            ExecStatement::Break => Flow::Break,
            ExecStatement::Continue => Flow::Continue,
            ExecStatement::While(mode, cond, block) => {
                self.interpret_while_statement(mode, cond, block)
            }
            ExecStatement::For(init, mode, cond, step, body) => {
                self.interpret_for_statement(init, mode, cond, step, body)
            }
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
            owning_func_index: None,
        };
        let addr_x = local_env.resolve_address(&id_x);
        assert_eq!(addr_x, 0, "x should be at address 0");

        let id_p = IdentifierRef {
            is_global: false,
            scope_depth: 0,
            local_index: 1,
            owning_func_index: None,
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

    // --- T1: 組み込み関数テスト ---

    fn create_test_env_with_stdin(stdin_data: &str) -> Environment {
        let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(
            stdin_data.as_bytes().to_vec(),
        )));
        let stdout_buf: Box<dyn std::io::Write> = Box::new(Vec::<u8>::new());
        Environment::new_with_config(stdin_cursor, stdout_buf, EnvironmentConfig::new())
    }

    fn get_stdout(env: &mut Environment) -> String {
        env.flush();
        let stdout = &env.stdout;
        // stdout は Box<dyn Write> なので、Vec<u8> にダウンキャストはできない。
        // interpret_func_with_io と同様の方法で取得する代わりに、
        // 共有バッファを使う方法を採用する。
        // ここでは unsafe でダウンキャストする。
        let ptr = &**stdout as *const dyn std::io::Write as *const Vec<u8>;
        let vec = unsafe { &*ptr };
        String::from_utf8(vec.clone()).unwrap()
    }

    #[test]
    fn test_builtin_trace() {
        let code = r#"
func: main() {
    __trace(1);
    __trace(1);
    __trace(2);
    return: 0;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(
            env.traced.get(&1),
            Some(&2),
            "__trace(1) should be called twice"
        );
        assert_eq!(
            env.traced.get(&2),
            Some(&1),
            "__trace(2) should be called once"
        );
    }

    #[test]
    fn test_builtin_assert_pass() {
        let code = r#"
func: main() {
    __assert(1);
    __assert(42);
    return: 0;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(0), "__assert with non-zero should not panic");
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_builtin_assert_fail() {
        let code = r#"
func: main() {
    __assert(0);
    return: 0;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        crate::interpreter::interpret_all(&mut env, &scope);
    }

    #[test]
    fn test_builtin_puti() {
        let code = r#"
func: main() {
    __puti(42);
    return: 0;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        crate::interpreter::interpret_all(&mut env, &scope);
        let output = get_stdout(&mut env);
        assert_eq!(output, "42", "__puti(42) should write '42' to stdout");
    }

    #[test]
    fn test_builtin_putc() {
        let code = r#"
func: main() {
    __putc(65);
    return: 0;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        crate::interpreter::interpret_all(&mut env, &scope);
        let output = get_stdout(&mut env);
        assert_eq!(output, "A", "__putc(65) should write 'A' to stdout");
    }

    #[test]
    fn test_builtin_geti() {
        let code = r#"
func: main() {
    return: __geti();
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env_with_stdin("42\n");
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(42), "__geti() should read 42 from stdin");
    }

    #[test]
    fn test_builtin_getc() {
        let code = r#"
func: main() {
    return: __getc();
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env_with_stdin("A");
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(65), "__getc() should read 'A' (65) from stdin");
    }

    // --- T2: 二項演算子テスト ---

    #[test]
    fn test_binary_add() {
        let code = "func: main() { return: 1 + 2; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(3));
    }

    #[test]
    fn test_binary_sub() {
        let code = "func: main() { return: 5 - 3; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_binary_mul() {
        let code = "func: main() { return: 3 * 4; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(12));
    }

    #[test]
    fn test_binary_div() {
        let code = "func: main() { return: 10 / 3; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(3));
    }

    #[test]
    fn test_binary_mod() {
        let code = "func: main() { return: 10 % 3; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_binary_equal() {
        let code = "func: main() { return: (3 == 3) + (3 == 4) * 10; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(1), "3==3 is 1, 3==4 is 0");
    }

    #[test]
    fn test_binary_not_equal() {
        let code = "func: main() { return: (3 != 4) + (3 != 3) * 10; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(1), "3!=4 is 1, 3!=3 is 0");
    }

    #[test]
    fn test_binary_less() {
        let code = "func: main() { return: (1 < 2) + (2 < 2) * 10 + (3 < 2) * 100; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(1), "1<2 is 1, 2<2 is 0, 3<2 is 0");
    }

    #[test]
    fn test_binary_less_equal() {
        let code = "func: main() { return: (1 <= 2) + (2 <= 2) * 10 + (3 <= 2) * 100; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(11), "1<=2 is 1, 2<=2 is 1, 3<=2 is 0");
    }

    #[test]
    fn test_binary_greater() {
        let code = "func: main() { return: (3 > 2) + (2 > 2) * 10 + (1 > 2) * 100; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(1), "3>2 is 1, 2>2 is 0, 1>2 is 0");
    }

    #[test]
    fn test_binary_greater_equal() {
        let code = "func: main() { return: (3 >= 2) + (2 >= 2) * 10 + (1 >= 2) * 100; }";
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(11), "3>=2 is 1, 2>=2 is 1, 1>=2 is 0");
    }

    #[test]
    fn test_binary_logical_and() {
        let code = r#"
func: main() {
    return: (1 && 1) + (1 && 0) * 10 + (0 && 1) * 100 + (0 && 0) * 1000;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(1), "1&&1=1, 1&&0=0, 0&&1=0, 0&&0=0");
    }

    #[test]
    fn test_binary_logical_or() {
        let code = r#"
func: main() {
    return: (1 || 1) + (1 || 0) * 10 + (0 || 1) * 100 + (0 || 0) * 1000;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(111), "1||1=1, 1||0=1, 0||1=1, 0||0=0");
    }

    // --- T3: 制御フローテスト ---

    #[test]
    fn test_if_else() {
        let code = r#"
func: main() {
    let: x;
    x = if:(1) { 10; } else: { 20; };
    let: y;
    y = if:(0) { 10; } else: { 20; };
    return: x + y * 100;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(2010), "if true -> 10, if false -> 20");
    }

    #[test]
    fn test_while_loop() {
        let code = r#"
func: main() {
    let: i; let: sum;
    i = 0; sum = 0;
    while:(i < 5) {
        sum = sum + i;
        i = i + 1;
    };
    return: sum;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(10), "sum of 0..4 = 10");
    }

    #[test]
    fn test_return_early() {
        let code = r#"
func: main() {
    return: 42;
    return: 99;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(42), "early return should return 42");
    }

    #[test]
    fn test_break_in_while() {
        let code = r#"
func: main() {
    let: i;
    i = 0;
    while:(1) {
        if:(i == 3) { break; } else: {};
        i = i + 1;
    };
    return: i;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(3), "break at i==3");
    }

    #[test]
    fn test_continue_in_while() {
        let code = r#"
func: main() {
    let: i; let: sum;
    i = 0; sum = 0;
    while:(i < 6) {
        i = i + 1;
        if:(i % 2 == 0) { continue; } else: {};
        sum = sum + i;
    };
    return: sum;
}
"#;
        let scope = parse_and_analyze(code);
        let mut env = create_test_env();
        let result = crate::interpreter::interpret_all(&mut env, &scope);
        assert_eq!(result, Some(9), "sum of odd numbers 1+3+5 = 9");
    }
}
