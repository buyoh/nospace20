//! NospaceVM — 式評価 (EvalCont 関連メソッド)

use super::*;

impl NospaceVM {
    // ─── EvalExpr ───

    pub(super) fn step_eval_expr(&mut self) -> ExecuteResult {
        let (expr_ptr, cont) = match self.frames.last_mut() {
            Some(Frame::EvalExpr { expr, cont }) => {
                let e = *expr;
                let c = std::mem::replace(cont, EvalCont::Start);
                (e, c)
            }
            _ => unreachable!(),
        };
        let expr = unsafe { &*expr_ptr };

        match cont {
            EvalCont::Start => {
                self.total_steps += 1;
                self.eval_start(expr)
            }
            EvalCont::AfterUnary(op) => {
                let v = self.value_stack.pop().unwrap_or(0);
                let r = crate::base::pure_eval::eval_unary_pure(&op, v)
                    .expect("unsupported unary op");
                self.finish_eval(r)
            }
            EvalCont::DerefAfter => {
                let addr = self.value_stack.pop().unwrap_or(0);
                let v = self.env.allocator.get(addr);
                self.finish_eval(v)
            }
            EvalCont::AfterArrayIndex { id_ref } => {
                let index = self.value_stack.pop().unwrap_or(0);
                let addr  = self.resolve_addr(&id_ref) + index;
                let v     = self.env.allocator.get(addr);
                self.finish_eval(v)
            }
            EvalCont::BinaryLeft { op, rhs } => {
                let left = self.value_stack.pop().unwrap_or(0);
                self.set_eval_cont(EvalCont::BinaryRight { op, left });
                self.frames.push(Frame::EvalExpr { expr: rhs, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            EvalCont::BinaryRight { op, left } => {
                let right = self.value_stack.pop().unwrap_or(0);
                // LogicalAnd/LogicalOr は eval_binary_pure が None を返すため直接処理
                let r = match &op {
                    Operator2::LogicalAnd => if left != 0 && right != 0 { 1 } else { 0 },
                    Operator2::LogicalOr => if left != 0 || right != 0 { 1 } else { 0 },
                    _ => crate::base::pure_eval::eval_binary_pure(&op, left, right)
                        .unwrap_or_else(|| panic!("runtime error: zero division {:?}", op)),
                };
                self.finish_eval(r)
            }
            EvalCont::AssignVar(id_ref) => {
                let v = self.value_stack.pop().unwrap_or(0);
                self.set_variable(&id_ref, v);
                self.finish_eval(v)
            }
            EvalCont::AssignArrIndex { id_ref, rhs } => {
                let index     = self.value_stack.pop().unwrap_or(0);
                let base_addr = self.resolve_addr(&id_ref) + index;
                self.set_eval_cont(EvalCont::AssignArrRhs { base_addr });
                self.frames.push(Frame::EvalExpr { expr: rhs, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            EvalCont::AssignArrRhs { base_addr } => {
                let v = self.value_stack.pop().unwrap_or(0);
                self.env.allocator.set(base_addr, v);
                self.finish_eval(v)
            }
            EvalCont::AssignDerefPtr { rhs } => {
                let addr = self.value_stack.pop().unwrap_or(0);
                self.set_eval_cont(EvalCont::AssignDerefRhs { addr });
                self.frames.push(Frame::EvalExpr { expr: rhs, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            EvalCont::AssignDerefRhs { addr } => {
                let v = self.value_stack.pop().unwrap_or(0);
                self.env.allocator.set(addr, v);
                self.finish_eval(v)
            }
            EvalCont::RefArrIndex(id_ref) => {
                let index = self.value_stack.pop().unwrap_or(0);
                let addr  = self.resolve_addr(&id_ref) + index;
                self.finish_eval(addr)
            }
            EvalCont::LogicalAndRhs(rhs) => {
                let left = self.value_stack.pop().unwrap_or(0);
                if left == 0 {
                    self.finish_eval(0)
                } else {
                    self.set_eval_cont(EvalCont::BinaryRight { op: Operator2::LogicalAnd, left });
                    self.frames.push(Frame::EvalExpr { expr: rhs, cont: EvalCont::Start });
                    ExecuteResult::Continue
                }
            }
            EvalCont::LogicalOrRhs(rhs) => {
                let left = self.value_stack.pop().unwrap_or(0);
                if left != 0 {
                    self.finish_eval(1)
                } else {
                    self.set_eval_cont(EvalCont::BinaryRight { op: Operator2::LogicalOr, left });
                    self.frames.push(Frame::EvalExpr { expr: rhs, cont: EvalCont::Start });
                    ExecuteResult::Continue
                }
            }
            EvalCont::UserFuncArgs { func_ref, args, next_arg, mut evaluated } => {
                let val = self.value_stack.pop().unwrap_or(0);
                evaluated.push(val);
                let args_ref = unsafe { &*args };
                if next_arg < args_ref.len() {
                    let ap: ExprPtr = args_ref[next_arg].as_ref() as *const _;
                    self.set_eval_cont(EvalCont::UserFuncArgs {
                        func_ref, args, next_arg: next_arg + 1, evaluated });
                    self.frames.push(Frame::EvalExpr { expr: ap, cont: EvalCont::Start });
                    ExecuteResult::Continue
                } else {
                    let fi = func_ref.local_index;
                    self.frames.pop(); // EvalExpr pop
                    self.push_func_frame(fi, &evaluated, false);
                    ExecuteResult::Continue
                }
            }
            EvalCont::BuiltinArgs { kind, args, next_arg, mut evaluated } => {
                let val = self.value_stack.pop().unwrap_or(0);
                evaluated.push(val);
                let args_ref = unsafe { &*args };
                if next_arg < args_ref.len() {
                    let ap: ExprPtr = args_ref[next_arg].as_ref() as *const _;
                    self.set_eval_cont(EvalCont::BuiltinArgs {
                        kind, args, next_arg: next_arg + 1, evaluated });
                    self.frames.push(Frame::EvalExpr { expr: ap, cont: EvalCont::Start });
                    ExecuteResult::Continue
                } else {
                    self.frames.pop();
                    let r = self.exec_builtin(kind, &evaluated);
                    self.value_stack.push(r);
                    ExecuteResult::Continue
                }
            }
            EvalCont::IfCond { mode, then_block, else_block } => {
                let cond_val = self.value_stack.pop().unwrap_or(0);
                let condition = match mode {
                    ConditionMode::NonZero  => cond_val != 0,
                    ConditionMode::Zero     => cond_val == 0,
                    ConditionMode::Negative => cond_val < 0,
                };
                let bp = if condition { then_block } else { else_block };
                let block = unsafe { &*bp };
                let scope_addr = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.frames.pop(); // EvalExpr pop
                self.frames.push(Frame::ExecBlock {
                    stmts, next_idx: 0, last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock { scope_addr, push_value: true },
                });
                ExecuteResult::Continue
            }
        }
    }

    pub(super) fn eval_start(&mut self, expr: &LocatedExecExpression) -> ExecuteResult {
        match &expr.expression {
            ExecExpression::Factor(v) => self.finish_eval(*v),
            ExecExpression::Variable(id) => {
                let v = self.get_variable(id);
                self.finish_eval(v)
            }
            ExecExpression::ArrayAccess(id_ref, index_expr, _) => {
                let ip: ExprPtr = index_expr.as_ref() as *const _;
                let id = *id_ref;
                self.set_eval_cont(EvalCont::AfterArrayIndex { id_ref: id });
                self.frames.push(Frame::EvalExpr { expr: ip, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            ExecExpression::Operation1(op, inner) => {
                match op {
                    Operator1::Ref => {
                        match &inner.expression {
                            ExecExpression::Variable(id) => {
                                let addr = self.resolve_addr(id);
                                self.finish_eval(addr)
                            }
                            ExecExpression::ArrayAccess(id_ref, index_expr, _) => {
                                let ip: ExprPtr = index_expr.as_ref() as *const _;
                                let id = *id_ref;
                                self.set_eval_cont(EvalCont::RefArrIndex(id));
                                self.frames.push(Frame::EvalExpr { expr: ip, cont: EvalCont::Start });
                                ExecuteResult::Continue
                            }
                            _ => panic!("runtime error: cannot take reference"),
                        }
                    }
                    Operator1::Deref => {
                        let ip: ExprPtr = inner.as_ref() as *const _;
                        self.set_eval_cont(EvalCont::DerefAfter);
                        self.frames.push(Frame::EvalExpr { expr: ip, cont: EvalCont::Start });
                        ExecuteResult::Continue
                    }
                    _ => {
                        let ip: ExprPtr = inner.as_ref() as *const _;
                        self.set_eval_cont(EvalCont::AfterUnary(op.clone()));
                        self.frames.push(Frame::EvalExpr { expr: ip, cont: EvalCont::Start });
                        ExecuteResult::Continue
                    }
                }
            }
            ExecExpression::Operation2(op, lhs, rhs) => {
                match op {
                    Operator2::LogicalAnd => {
                        let rp: ExprPtr = rhs.as_ref() as *const _;
                        let lp: ExprPtr = lhs.as_ref() as *const _;
                        self.set_eval_cont(EvalCont::LogicalAndRhs(rp));
                        self.frames.push(Frame::EvalExpr { expr: lp, cont: EvalCont::Start });
                        ExecuteResult::Continue
                    }
                    Operator2::LogicalOr => {
                        let rp: ExprPtr = rhs.as_ref() as *const _;
                        let lp: ExprPtr = lhs.as_ref() as *const _;
                        self.set_eval_cont(EvalCont::LogicalOrRhs(rp));
                        self.frames.push(Frame::EvalExpr { expr: lp, cont: EvalCont::Start });
                        ExecuteResult::Continue
                    }
                    Operator2::Assign => self.push_assign(lhs, rhs),
                    _ => {
                        let lp: ExprPtr = lhs.as_ref() as *const _;
                        let rp: ExprPtr = rhs.as_ref() as *const _;
                        self.set_eval_cont(EvalCont::BinaryLeft { op: op.clone(), rhs: rp });
                        self.frames.push(Frame::EvalExpr { expr: lp, cont: EvalCont::Start });
                        ExecuteResult::Continue
                    }
                }
            }
            ExecExpression::BuiltinFunction(kind, args) => {
                if args.is_empty() {
                    self.frames.pop();
                    let r = self.exec_builtin(*kind, &[]);
                    self.value_stack.push(r);
                    return ExecuteResult::Continue;
                }
                let ap: ArgsPtr = args as *const _;
                let fp: ExprPtr = args[0].as_ref() as *const _;
                self.set_eval_cont(EvalCont::BuiltinArgs { kind: *kind, args: ap, next_arg: 1, evaluated: Vec::new() });
                self.frames.push(Frame::EvalExpr { expr: fp, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            ExecExpression::UserFunction(func_ref, args) => {
                let fi = func_ref.local_index;
                if args.is_empty() {
                    self.frames.pop();
                    self.push_func_frame(fi, &[], false);
                    return ExecuteResult::Continue;
                }
                let ap: ArgsPtr = args as *const _;
                let fp: ExprPtr = args[0].as_ref() as *const _;
                self.set_eval_cont(EvalCont::UserFuncArgs { func_ref: *func_ref, args: ap, next_arg: 1, evaluated: Vec::new() });
                self.frames.push(Frame::EvalExpr { expr: fp, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            ExecExpression::If(mode, cond, then_block, else_block) => {
                let cp: ExprPtr  = cond.as_ref() as *const _;
                let tp: BlockPtr = then_block as *const _;
                let ep: BlockPtr = else_block as *const _;
                self.set_eval_cont(EvalCont::IfCond { mode: *mode, then_block: tp, else_block: ep });
                self.frames.push(Frame::EvalExpr { expr: cp, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            ExecExpression::Block(block) => {
                let scope_addr = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.frames.pop();
                self.frames.push(Frame::ExecBlock {
                    stmts, next_idx: 0, last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock { scope_addr, push_value: true },
                });
                ExecuteResult::Continue
            }
            ExecExpression::InternalBuiltinFunction(kind) => {
                let r = self.exec_internal_builtin(kind);
                self.finish_eval(r)
            }
        }
    }

    pub(super) fn push_assign(&mut self, lhs: &LocatedExecExpression, rhs: &LocatedExecExpression) -> ExecuteResult {
        let rp: ExprPtr = rhs as *const _;
        match &lhs.expression {
            ExecExpression::Variable(id_ref) => {
                let id = *id_ref;
                self.set_eval_cont(EvalCont::AssignVar(id));
                self.frames.push(Frame::EvalExpr { expr: rp, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            ExecExpression::ArrayAccess(id_ref, index_expr, _) => {
                let ip: ExprPtr = index_expr.as_ref() as *const _;
                let id = *id_ref;
                self.set_eval_cont(EvalCont::AssignArrIndex { id_ref: id, rhs: rp });
                self.frames.push(Frame::EvalExpr { expr: ip, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            ExecExpression::Operation1(Operator1::Deref, inner) => {
                let ip: ExprPtr = inner.as_ref() as *const _;
                self.set_eval_cont(EvalCont::AssignDerefPtr { rhs: rp });
                self.frames.push(Frame::EvalExpr { expr: ip, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            _ => panic!("runtime error: left value is not assignable"),
        }
    }

    pub(super) fn finish_eval(&mut self, value: i64) -> ExecuteResult {
        self.frames.pop();
        self.value_stack.push(value);
        ExecuteResult::Continue
    }

    pub(super) fn set_eval_cont(&mut self, cont: EvalCont) {
        if let Some(Frame::EvalExpr { cont: c, .. }) = self.frames.last_mut() { *c = cont; }
    }
}
