use crate::{
    semantic_analyzer::{
        Block, ConditionMode, ExecExpression, ExecStatement, Function, IdentifierRef,
        InternalBuiltinFunctionKind, LocatedExecExpression, LocatedExecStatement, Scope,
    },
    tree_parser::{Operator1, Operator2},
};

use super::environment::Environment;
use super::types::{try_expr, ExpressionFlow, Flow};
use crate::base::pure_eval;

use std::cell::RefCell;

thread_local! {
    static UNINIT_COUNTER: RefCell<u64> = RefCell::new(0);
}

/// 未初期化変数のフィル値を生成する（決定論的な非自明値）
///
/// 0 でない値を返すことで、初期値 0 への暗黙依存バグを検出しやすくする。
/// スレッドローカルなカウンタを使い、外部 crate なしで生成する。
#[allow(dead_code)]
pub(super) fn random_uninit_value() -> i64 {
    UNINIT_COUNTER.with(|c| {
        let mut count = c.borrow_mut();
        *count = count.wrapping_add(1);
        // 簡易ハッシュ: 0 を避けつつ決定論的な非自明値を生成
        crate::algorithm::hash::lcg_hash(*count) as i64
    })
}

/// 指定サイズの未初期化変数ベクタを生成する
///
/// `randomize` が true のときランダム値、false のとき 0 で初期化する。
#[allow(dead_code)]
pub(super) fn create_uninit_vec(size: usize, randomize: bool) -> Vec<i64> {
    if randomize {
        (0..size).map(|_| random_uninit_value()).collect()
    } else {
        vec![0; size]
    }
}

/// 1つのfunction scopeの`実行時インスタンス`を管理する
///
/// scope_stack を Vec<i64>(アロケータアドレス) に変更。
/// 変数アクセスを O(1) にするため、IdentifierRef を使用してインデックスベースでアクセスする。
pub(super) struct LocalEnvironment<'a, 'aenv> {
    pub(super) env: &'aenv mut Environment,
    pub(super) root_scope: &'a Scope,
    /// スコープスタック: 末尾が現在のスコープアドレス（アロケータ上のベースアドレス）
    pub(super) scope_stack: Vec<i64>,
}

impl LocalEnvironment<'_, '_> {
    pub(super) fn new_func<'a, 'aenv>(
        env: &'aenv mut Environment,
        root_scope: &'a Scope,
        func: &'a Function,
        args: &Vec<i64>,
    ) -> LocalEnvironment<'a, 'aenv> {
        // アロケータ上に関数スコープ分の領域を確保
        let base_addr = env
            .allocator
            .alloc_internal_uninit(func.block.scope.variable_count, env.config.randomize_uninit);

        // 引数を対応する変数にセット（最適化: 事前計算されたインデックスを使用）
        for (i, arg_val) in args.iter().enumerate() {
            if i < func.arg_indices.len() {
                env.allocator
                    .set(base_addr + func.arg_indices[i] as i64, *arg_val);
            }
        }

        LocalEnvironment {
            env,
            root_scope,
            scope_stack: vec![base_addr],
        }
    }

    /// ブロックに入る
    fn enter_block(&mut self, scope: &Scope) {
        // アロケータ上にスコープ分の領域を確保（randomize_uninit モードではランダム値で初期化）
        let base_addr = self
            .env
            .allocator
            .alloc_internal_uninit(scope.variable_count, self.env.config.randomize_uninit);
        self.scope_stack.push(base_addr);
    }

    /// ブロックから出る
    fn leave_block(&mut self) {
        let base_addr = self.scope_stack.pop().unwrap();
        self.env.allocator.free_internal(base_addr);
    }

    /// 識別子参照から値を取得
    /// グローバル変数対応（is_global フラグチェック）
    fn get_variable(&self, id: &IdentifierRef) -> i64 {
        let addr = if id.is_global {
            self.env.global_base_addr + id.local_index as i64
        } else {
            let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
            self.scope_stack[scope_idx] + id.local_index as i64
        };
        self.env.allocator.get(addr)
    }

    /// 識別子参照に値を設定
    /// グローバル変数対応（is_global フラグチェック）
    fn set_variable(&mut self, id: &IdentifierRef, value: i64) {
        let addr = if id.is_global {
            self.env.global_base_addr + id.local_index as i64
        } else {
            let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
            self.scope_stack[scope_idx] + id.local_index as i64
        };
        self.env.allocator.set(addr, value);
    }

    /// IdentifierRef から絶対アドレスを計算
    /// Phase 2+3: アロケータアドレスをそのまま返す (O(1))
    fn resolve_address(&self, id: &IdentifierRef) -> i64 {
        if id.is_global {
            self.env.global_base_addr + id.local_index as i64
        } else {
            let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
            self.scope_stack[scope_idx] + id.local_index as i64
        }
    }

    /// 絶対アドレスから値を取得
    fn get_by_address(&self, addr: i64) -> i64 {
        self.env.allocator.get(addr)
    }

    /// 絶対アドレスに値を設定
    fn set_by_address(&mut self, addr: i64, value: i64) {
        self.env.allocator.set(addr, value);
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
            BuiltinFunctionKind::Alloc => {
                let size = try_expr!(self.interpret_expression(args.first().unwrap()));
                let ptr = self.env.allocator.alloc(size);
                ExpressionFlow::Value(ptr)
            }
            BuiltinFunctionKind::Free => {
                let ptr = try_expr!(self.interpret_expression(args.first().unwrap()));
                self.env.allocator.free(ptr);
                ExpressionFlow::Value(0)
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

        // アロケータ上に新しいスコープ分の領域を確保（randomize_uninit モードではランダム値で初期化）
        let base_addr = self.env.allocator.alloc_internal_uninit(
            func.block.scope.variable_count,
            self.env.config.randomize_uninit,
        );

        // static 変数があり、永続ストレージが存在する場合は値を復元
        // 関数インデックスをキーとして使用
        let func_key = func_ref.local_index;
        if has_static {
            if let Some(&static_addr) = self.env.function_static_addrs.get(&func_key) {
                for var in &func.block.scope.variables {
                    if var.is_static {
                        let slot_idx = var.slot_index;
                        let slot_count = var.array_size.unwrap_or(1);
                        for i in 0..slot_count {
                            let val = self.env.allocator.get(static_addr + (slot_idx + i) as i64);
                            self.env
                                .allocator
                                .set(base_addr + (slot_idx + i) as i64, val);
                        }
                    }
                }
            }
        }

        for (i, arg_val) in arg_values.iter().enumerate() {
            if i < func.arg_indices.len() {
                self.env
                    .allocator
                    .set(base_addr + func.arg_indices[i] as i64, *arg_val);
            }
        }
        self.scope_stack.push(base_addr);

        // 既存の LocalEnvironment 上で関数本体を実行
        let result = match self.interpret_statements(&func.block.statements) {
            Flow::Proceed => ExpressionFlow::Value(0),
            Flow::Return(x) => ExpressionFlow::Value(x),
            Flow::Continue => panic!("internal error: unexpected continue"),
            Flow::Break => panic!("internal error: unexpected break"),
        };

        // Phase 4: static 変数の値を永続ストレージに保存
        if has_static {
            if let Some(&static_addr) = self.env.function_static_addrs.get(&func_key) {
                let base_addr = *self.scope_stack.last().unwrap();
                for var in &func.block.scope.variables {
                    if var.is_static {
                        let slot_idx = var.slot_index;
                        let slot_count = var.array_size.unwrap_or(1);
                        for i in 0..slot_count {
                            let val = self.env.allocator.get(base_addr + (slot_idx + i) as i64);
                            self.env
                                .allocator
                                .set(static_addr + (slot_idx + i) as i64, val);
                        }
                    }
                }
            }
        }

        // 関数スコープを pop して解放
        let base_addr = self.scope_stack.pop().unwrap();
        self.env.allocator.free_internal(base_addr);
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
                    ExecExpression::Variable(id_ref, _) => {
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
                let res = pure_eval::eval_unary_pure(op, v1)
                    .expect("unreachable: unsupported unary operation");
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
                ExecExpression::Variable(id_ref, _) => {
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
            return ExpressionFlow::Value(pure_eval::bool_to_int(v2 != 0));
        }
        // 論理OR: 短絡評価 (左辺が非0なら右辺を評価せず1を返す)
        if let Operator2::LogicalOr = op {
            let v1 = try_expr!(self.interpret_expression(expr1));
            if v1 != 0 {
                return ExpressionFlow::Value(1);
            }
            let v2 = try_expr!(self.interpret_expression(expr2));
            return ExpressionFlow::Value(pure_eval::bool_to_int(v2 != 0));
        }
        let v1 = try_expr!(self.interpret_expression(expr1));
        let v2 = try_expr!(self.interpret_expression(expr2));
        let res = pure_eval::eval_binary_pure(op, v1, v2).unwrap_or_else(|| {
            panic!(
                "runtime error: zero division or unsupported operation {:?}",
                op
            )
        });
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
            ExecExpression::Variable(id_ref, value_type) => {
                let value = match value_type {
                    crate::semantic_analyzer::ValueType::Struct(_) => self.resolve_address(id_ref),
                    _ => self.get_variable(id_ref),
                };
                ExpressionFlow::Value(value)
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
            ExecExpression::TypeAssertion(inner, _) => self.interpret_expression(inner),
            ExecExpression::VoidCast(inner) => {
                let _ = try_expr!(self.interpret_expression(inner));
                ExpressionFlow::Value(0)
            }
            ExecExpression::StructFieldAccess(base, offset, _, field_type) => {
                let base_addr = try_expr!(self.interpret_expression(base));
                let addr = base_addr + *offset as i64;
                match field_type {
                    crate::semantic_analyzer::ValueType::Struct(_) => ExpressionFlow::Value(addr),
                    _ => ExpressionFlow::Value(self.get_by_address(addr)),
                }
            }
            ExecExpression::StructFieldArrayAccess(base, offset, idx, _) => {
                let base_addr = try_expr!(self.interpret_expression(base));
                let index = try_expr!(self.interpret_expression(idx));
                ExpressionFlow::Value(self.get_by_address(base_addr + *offset as i64 + index))
            }
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
#[path = "exec_tests.rs"]
mod tests;
