//! NospaceVM — ステートメント実行・ループ実行

use super::*;

impl NospaceVM {
    // ─── ExecBlock ───

    pub(super) fn step_exec_block(&mut self) -> ExecuteResult {
        let (stmts_ptr, next_idx, last_value, waiting) = match self.frames.last() {
            Some(Frame::ExecBlock {
                stmts,
                next_idx,
                last_value,
                waiting,
                ..
            }) => (*stmts, *next_idx, *last_value, waiting.clone()),
            _ => unreachable!(),
        };

        match waiting {
            ExecBlockWait::WaitExpr => {
                let val = self.value_stack.pop().unwrap_or(0);
                if let Some(Frame::ExecBlock {
                    last_value: lv,
                    waiting: w,
                    ..
                }) = self.frames.last_mut()
                {
                    *lv = val;
                    *w = ExecBlockWait::None;
                }
                return ExecuteResult::Continue;
            }
            ExecBlockWait::WaitReturn => {
                let val = self.value_stack.pop().unwrap_or(0);
                // frames.pop() しない: propagate_flow に任せる
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::None;
                }
                self.flow = Some(FlowControl::Return(val));
                return ExecuteResult::Continue;
            }
            ExecBlockWait::WaitStmt => {
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::None;
                }
                return ExecuteResult::Continue;
            }
            ExecBlockWait::None => {}
        }

        let stmts = unsafe { &*stmts_ptr };
        if next_idx >= stmts.len() {
            return self.finish_exec_block(last_value);
        }

        if let Some(Frame::ExecBlock { next_idx: ni, .. }) = self.frames.last_mut() {
            *ni += 1;
        }

        let stmt = &stmts[next_idx].statement;
        match stmt {
            ExecStatement::Expression(expr) => {
                let ep: ExprPtr = expr.as_ref() as *const _;
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::WaitExpr;
                }
                self.frames.push(Frame::EvalExpr {
                    expr: ep,
                    cont: EvalCont::Start,
                });
                ExecuteResult::Continue
            }
            ExecStatement::Return(None) => {
                // propagate_flow が ScopeBlock や UserFunc/MainFunc のクリーンアップを行う
                self.flow = Some(FlowControl::Return(0));
                ExecuteResult::Continue
            }
            ExecStatement::Return(Some(expr)) => {
                let ep: ExprPtr = expr.as_ref() as *const _;
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::WaitReturn;
                }
                self.frames.push(Frame::EvalExpr {
                    expr: ep,
                    cont: EvalCont::Start,
                });
                ExecuteResult::Continue
            }
            ExecStatement::Break => {
                // propagate_flow がループフレームまでの ScopeBlock をクリーンアップする
                self.flow = Some(FlowControl::Break);
                ExecuteResult::Continue
            }
            ExecStatement::Continue => {
                // propagate_flow がループフレームまでの ScopeBlock をクリーンアップする
                self.flow = Some(FlowControl::Continue);
                ExecuteResult::Continue
            }
            ExecStatement::While(mode, cond, body) => {
                let cp: ExprPtr = cond.as_ref() as *const _;
                let bp: BlockPtr = body as *const _;
                let m = *mode;
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::WaitStmt;
                }
                self.frames.push(Frame::WhileLoop {
                    mode: m,
                    cond: cp,
                    body: bp,
                    phase: WhilePhase::EvalCond,
                });
                ExecuteResult::Continue
            }
            ExecStatement::For(init, mode, cond, step, body) => {
                let ib: BlockPtr = init as *const _;
                let cb: BlockPtr = cond as *const _;
                let sb: BlockPtr = step as *const _;
                let bb: BlockPtr = body as *const _;
                let m = *mode;
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::WaitStmt;
                }
                self.frames.push(Frame::ForLoop {
                    mode: m,
                    init_block: ib,
                    cond_block: cb,
                    step_block: sb,
                    body_block: bb,
                    phase: ForPhase::StartInit,
                });
                ExecuteResult::Continue
            }
        }
    }

    pub(super) fn finish_exec_block(&mut self, last_value: i64) -> ExecuteResult {
        let frame = self.frames.pop().unwrap();
        match frame {
            Frame::ExecBlock {
                completion:
                    BlockCompletion::MainFunc {
                        func_idx,
                        scope_addr,
                    },
                ..
            } => {
                self.save_static_vars(func_idx, scope_addr);
                self.leave_scope(scope_addr);
                ExecuteResult::Complete(Some(last_value))
            }
            Frame::ExecBlock {
                completion:
                    BlockCompletion::UserFunc {
                        func_idx,
                        scope_addr,
                    },
                ..
            } => {
                self.save_static_vars(func_idx, scope_addr);
                self.leave_scope(scope_addr);
                self.value_stack.push(last_value);
                ExecuteResult::Continue
            }
            Frame::ExecBlock {
                completion:
                    BlockCompletion::ScopeBlock {
                        scope_addr,
                        push_value,
                    },
                ..
            } => {
                self.leave_scope(scope_addr);
                if push_value {
                    self.value_stack.push(last_value);
                }
                ExecuteResult::Continue
            }
            Frame::ExecBlock {
                completion: BlockCompletion::GlobalStmts,
                ..
            } => ExecuteResult::Continue,
            _ => ExecuteResult::Continue,
        }
    }

    // ─── WhileLoop ───

    pub(super) fn step_while(&mut self) -> ExecuteResult {
        let (mode, cond, body, phase) = match self.frames.last_mut() {
            Some(Frame::WhileLoop {
                mode,
                cond,
                body,
                phase,
            }) => (
                *mode,
                *cond,
                *body,
                std::mem::replace(phase, WhilePhase::EvalCond),
            ),
            _ => unreachable!(),
        };
        match phase {
            WhilePhase::EvalCond => {
                self.set_while_phase(WhilePhase::CheckCond);
                self.frames.push(Frame::EvalExpr {
                    expr: cond,
                    cont: EvalCont::Start,
                });
                ExecuteResult::Continue
            }
            WhilePhase::CheckCond => {
                let cv = self.value_stack.pop().unwrap_or(0);
                let ok = match mode {
                    ConditionMode::NonZero => cv != 0,
                    ConditionMode::Zero => cv == 0,
                    ConditionMode::Negative => cv < 0,
                };
                if !ok {
                    self.frames.pop();
                    return ExecuteResult::Continue;
                }
                let block = unsafe { &*body };
                let sa = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.set_while_phase(WhilePhase::WaitBody);
                self.frames.push(Frame::ExecBlock {
                    stmts,
                    next_idx: 0,
                    last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock {
                        scope_addr: sa,
                        push_value: false,
                    },
                });
                ExecuteResult::Continue
            }
            WhilePhase::WaitBody => {
                if let Some(FlowControl::Break) = &self.flow {
                    self.flow = None;
                    self.frames.pop();
                    return ExecuteResult::Continue;
                }
                if let Some(FlowControl::Continue) = &self.flow {
                    self.flow = None;
                }
                self.set_while_phase(WhilePhase::EvalCond);
                ExecuteResult::Continue
            }
        }
    }

    fn set_while_phase(&mut self, p: WhilePhase) {
        if let Some(Frame::WhileLoop { phase, .. }) = self.frames.last_mut() {
            *phase = p;
        }
    }

    // ─── ForLoop ───

    pub(super) fn step_for(&mut self) -> ExecuteResult {
        let (mode, ib, cb, sb, bb, phase) = match self.frames.last_mut() {
            Some(Frame::ForLoop {
                mode,
                init_block,
                cond_block,
                step_block,
                body_block,
                phase,
            }) => (
                *mode,
                *init_block,
                *cond_block,
                *step_block,
                *body_block,
                std::mem::replace(phase, ForPhase::StartInit),
            ),
            _ => unreachable!(),
        };

        match phase {
            ForPhase::StartInit => {
                // init ブロックのスコープは for ループ全体で維持する必要がある
                // ScopeBlock ではなく GlobalStmts で push し、leave_scope を回避する
                let block = unsafe { &*ib };
                let sa = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                // set_for_phase を push の前に呼ぶ
                // （push 後は frames.last() が ExecBlock になり ForLoop に届かない）
                self.set_for_phase(ForPhase::WaitInit {
                    init_scope_addr: sa,
                });
                self.frames.push(Frame::ExecBlock {
                    stmts,
                    next_idx: 0,
                    last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::GlobalStmts,
                });
                ExecuteResult::Continue
            }
            ForPhase::WaitInit { init_scope_addr } => {
                // GlobalStmts 完了後、init スコープは scope_stack 上にそのまま残っている
                self.set_for_phase(ForPhase::StartCond { init_scope_addr });
                ExecuteResult::Continue
            }
            ForPhase::StartCond { init_scope_addr } => {
                let block = unsafe { &*cb };
                let sa = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.set_for_phase(ForPhase::WaitCond { init_scope_addr });
                self.frames.push(Frame::ExecBlock {
                    stmts,
                    next_idx: 0,
                    last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock {
                        scope_addr: sa,
                        push_value: true,
                    },
                });
                ExecuteResult::Continue
            }
            ForPhase::WaitCond { init_scope_addr } => {
                self.set_for_phase(ForPhase::CheckCond { init_scope_addr });
                ExecuteResult::Continue
            }
            ForPhase::CheckCond { init_scope_addr } => {
                let cv = self.value_stack.pop().unwrap_or(0);
                let ok = match mode {
                    ConditionMode::NonZero => cv != 0,
                    ConditionMode::Zero => cv == 0,
                    ConditionMode::Negative => cv < 0,
                };
                if !ok {
                    if self.scope_stack.last() == Some(&init_scope_addr) {
                        self.scope_stack.pop();
                    }
                    self.env.allocator.free_internal(init_scope_addr);
                    self.frames.pop();
                    return ExecuteResult::Continue;
                }
                self.set_for_phase(ForPhase::StartBody { init_scope_addr });
                ExecuteResult::Continue
            }
            ForPhase::StartBody { init_scope_addr } => {
                let block = unsafe { &*bb };
                let sa = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.set_for_phase(ForPhase::WaitBody { init_scope_addr });
                self.frames.push(Frame::ExecBlock {
                    stmts,
                    next_idx: 0,
                    last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock {
                        scope_addr: sa,
                        push_value: false,
                    },
                });
                ExecuteResult::Continue
            }
            ForPhase::WaitBody { init_scope_addr } => {
                if let Some(FlowControl::Break) = &self.flow {
                    self.flow = None;
                    if self.scope_stack.last() == Some(&init_scope_addr) {
                        self.scope_stack.pop();
                    }
                    self.env.allocator.free_internal(init_scope_addr);
                    self.frames.pop();
                    return ExecuteResult::Continue;
                }
                if let Some(FlowControl::Continue) = &self.flow {
                    self.flow = None;
                }
                self.set_for_phase(ForPhase::StartStep { init_scope_addr });
                ExecuteResult::Continue
            }
            ForPhase::StartStep { init_scope_addr } => {
                let block = unsafe { &*sb };
                let sa = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.set_for_phase(ForPhase::WaitStep { init_scope_addr });
                self.frames.push(Frame::ExecBlock {
                    stmts,
                    next_idx: 0,
                    last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock {
                        scope_addr: sa,
                        push_value: false,
                    },
                });
                ExecuteResult::Continue
            }
            ForPhase::WaitStep { init_scope_addr } => {
                if let Some(FlowControl::Break) = &self.flow {
                    self.flow = None;
                    if self.scope_stack.last() == Some(&init_scope_addr) {
                        self.scope_stack.pop();
                    }
                    self.env.allocator.free_internal(init_scope_addr);
                    self.frames.pop();
                    return ExecuteResult::Continue;
                }
                self.set_for_phase(ForPhase::StartCond { init_scope_addr });
                ExecuteResult::Continue
            }
        }
    }

    fn set_for_phase(&mut self, p: ForPhase) {
        if let Some(Frame::ForLoop { phase, .. }) = self.frames.last_mut() {
            *phase = p;
        }
    }
}
