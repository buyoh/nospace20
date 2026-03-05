//! # NospaceVM — 中断・再開可能な nospace インタプリタ
//!
//! `WhitespaceVM` と同等のインターフェースを持つ明示的スタックマシン実装。
//! `step(budget)` で指定ステップ数だけ実行し、任意のタイミングで中断・再開できる。
//!
//! ## 設計
//!
//! - `Scope` を所有し、フレームから AST への参照は **raw pointer** で行う（WASM 向けライフタイムフリー）
//! - `scope` は構築後に move されないため、raw pointer の有効性は `NospaceVM` 生存期間中保証される
//! - 既存の再帰インタプリタ (`exec.rs`) は変更せず維持

mod eval;
mod exec;
mod scope;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::rc::Rc;

use crate::base::error::NospaceError;
use crate::base::shared_writer::SharedWriter;
use crate::semantic_analyzer::{
    Block, BuiltinFunctionKind, ConditionMode, ExecExpression, ExecStatement, IdentifierRef,
    InternalBuiltinFunctionKind, LocatedExecExpression, LocatedExecStatement, Scope,
};
use crate::tree_parser::{Operator1, Operator2};

use super::environment::{Environment, EnvironmentConfig};
use super::InterpretError;

// ===== Raw pointer 型エイリアス =====
// Safety: `NospaceVM` が `scope` を所有し、scope は move/mutate しないため
// これらのポインタは NospaceVM 生存期間中有効

pub(super) type StmtsPtr = *const Vec<LocatedExecStatement>;
pub(super) type ExprPtr = *const LocatedExecExpression;
pub(super) type ArgsPtr = *const Vec<Box<LocatedExecExpression>>;
pub(super) type BlockPtr = *const Block;

// ===== 公開型定義 =====

/// nospace インタプリタの実行結果
#[derive(Debug)]
pub enum StepResult {
    /// 実行継続中（バジェット消費で中断）
    Suspended,
    /// 正常終了
    Complete {
        return_value: Option<i64>,
    },
    /// 実行時エラー
    Error(InterpretError),
}

// ===== 内部型定義 =====

/// 制御フローの種別（return/break/continue）
#[derive(Clone, Debug)]
pub(super) enum FlowControl {
    Return(i64),
    Break,
    Continue,
}

/// グローバル初期化のフェーズ（static 変数初期化 → ルート文実行 → main 呼出し）
pub(super) enum GlobalInitPhase {
    AllocGlobals,
    ExecRootStaticInit { stmt_idx: usize },
    ExecFuncStaticInits { func_idx: usize },
    ExecFuncStaticStmt { func_idx: usize, static_addr: i64, stmt_idx: usize },
    ExecRootStmts { stmt_idx: usize },
    CallMain,
}

/// ExecBlock 完了時のアクション（スコープ解放・値 push の制御）
pub(super) enum BlockCompletion {
    MainFunc  { func_idx: usize, scope_addr: i64 },
    UserFunc  { func_idx: usize, scope_addr: i64 },
    ScopeBlock { scope_addr: i64, push_value: bool },
    GlobalStmts,
}

/// ExecBlock のサブフレーム待機状態（式評価中・return 待ち等）
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExecBlockWait {
    None,
    WaitExpr,
    WaitStmt,
    WaitReturn,
}

/// 式評価の継続状態
pub(super) enum EvalCont {
    Start,
    AfterUnary(Operator1),
    DerefAfter,
    AfterArrayIndex { id_ref: IdentifierRef },
    BinaryLeft  { op: Operator2, rhs: ExprPtr },
    BinaryRight { op: Operator2, left: i64 },
    AssignVar(IdentifierRef),
    AssignArrIndex { id_ref: IdentifierRef, rhs: ExprPtr },
    AssignArrRhs   { base_addr: i64 },
    AssignDerefPtr { rhs: ExprPtr },
    AssignDerefRhs { addr: i64 },
    RefArrIndex(IdentifierRef),
    LogicalAndRhs(ExprPtr),
    LogicalOrRhs(ExprPtr),
    UserFuncArgs {
        func_ref:  IdentifierRef,
        args:      ArgsPtr,
        next_arg:  usize,
        evaluated: Vec<i64>,
    },
    BuiltinArgs {
        kind:      BuiltinFunctionKind,
        args:      ArgsPtr,
        next_arg:  usize,
        evaluated: Vec<i64>,
    },
    IfCond {
        mode:       ConditionMode,
        then_block: BlockPtr,
        else_block: BlockPtr,
    },
}

/// while ループの実行フェーズ（条件評価 → チェック → ボディ実行）
pub(super) enum WhilePhase {
    EvalCond,
    CheckCond,
    WaitBody,
}

/// for ループの実行フェーズ（init → cond → body → step の繰り返し）
pub(super) enum ForPhase {
    StartInit,
    WaitInit  { init_scope_addr: i64 },
    StartCond { init_scope_addr: i64 },
    WaitCond  { init_scope_addr: i64 },
    CheckCond { init_scope_addr: i64 },
    StartBody { init_scope_addr: i64 },
    WaitBody  { init_scope_addr: i64 },
    StartStep { init_scope_addr: i64 },
    WaitStep  { init_scope_addr: i64 },
}

/// 実行フレーム
///
/// フレームスタックの末尾が現在実行中のフレーム。
/// raw pointer は `NospaceVM` が `scope` を所有している間有効。
pub(super) enum Frame {
    GlobalInit { phase: GlobalInitPhase },
    ExecBlock {
        stmts:     StmtsPtr,
        next_idx:  usize,
        last_value: i64,
        waiting:   ExecBlockWait,
        completion: BlockCompletion,
    },
    EvalExpr { expr: ExprPtr, cont: EvalCont },
    WhileLoop {
        mode:  ConditionMode,
        cond:  ExprPtr,
        body:  BlockPtr,
        phase: WhilePhase,
    },
    ForLoop {
        mode:       ConditionMode,
        init_block: BlockPtr,
        cond_block: BlockPtr,
        step_block: BlockPtr,
        body_block: BlockPtr,
        phase:      ForPhase,
    },
}

/// `execute_one_step` の戻り値（VM 内部使用、Continue/Complete/Error）
pub(super) enum ExecuteResult {
    Continue,
    Complete(Option<i64>),
    Error(InterpretError),
}

// ===== NospaceVM 本体 =====

/// nospace ステップ実行 VM
///
/// 明示的スタックマシンとして全実行状態を保持する。
/// `step()` / `run()` で指定ステップずつ実行し、任意のタイミングで中断・再開可能。
pub struct NospaceVM {
    pub(super) scope:          Scope,
    pub(super) frames:         Vec<Frame>,
    pub(super) value_stack:    Vec<i64>,
    pub(super) scope_stack:    Vec<i64>,
    pub(super) flow:           Option<FlowControl>,
    pub(super) env:            Environment,
    pub(super) stdout_capture: Option<Rc<RefCell<Vec<u8>>>>,
    pub(super) total_steps:    usize,
    traced:         BTreeMap<i64, i64>,
    pub(super) completed:      bool,
    pub(super) return_value:   Option<i64>,
}

impl NospaceVM {
    // ===== コンストラクタ =====

    /// nospace ソースコードから VM を構築する
    ///
    /// パース → 意味解析 → VM 構築を一括実行する。
    /// エラーの場合は `NospaceError` を返す（パースエラーを含む）。
    pub fn from_source(source: &str) -> Result<Self, NospaceError> {
        let tokens = crate::token_parser::parse_to_tokens(&source.to_string())?;
        let tree = crate::tree_parser::parse_to_tree(&tokens)?;
        let scope = crate::semantic_analyzer::analyze(&tree)?;
        Self::from_scope(scope).map_err(NospaceError::Interpret)
    }

    /// 解析済み `Scope` から VM を構築する
    ///
    /// `Scope` を所有し、初期フレームをスタックに積む。
    pub fn from_scope(scope: Scope) -> Result<Self, InterpretError> {
        // stdout キャプチャバッファを初期化
        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_writer: Box<dyn Write> =
            Box::new(SharedWriter(Rc::clone(&stdout_buf)));
        let stdin: Box<dyn BufRead> =
            Box::new(BufReader::new(Cursor::new(Vec::<u8>::new())));

        let env = Environment::new_with_buffers(stdin, stdout_writer);

        Ok(Self {
            scope,
            frames: vec![Frame::GlobalInit { phase: GlobalInitPhase::AllocGlobals }],
            value_stack: Vec::new(),
            scope_stack: Vec::new(),
            flow: None,
            env,
            stdout_capture: Some(stdout_buf),
            total_steps: 0,
            traced: BTreeMap::new(),
            completed: false,
            return_value: None,
        })
    }

    // ===== Builder パターン =====

    /// stdin を設定する（stdout はキャプチャバッファを維持）
    pub fn with_stdin(mut self, stdin: Box<dyn BufRead>) -> Self {
        self.env.stdin = stdin;
        self
    }

    /// I/O バッファを明示指定して構築する
    ///
    /// stdout を外部バッファに設定した場合、`get_stdout_string()` ではなく
    /// 呼び出し元のバッファから直接 stdout を取得すること。
    pub fn with_io(mut self, stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self {
        self.env.stdin = stdin;
        self.env.stdout = stdout;
        // 外部 stdout を使用するためキャプチャを無効化
        self.stdout_capture = None;
        self
    }

    /// `EnvironmentConfig` を設定する
    pub fn with_config(mut self, config: EnvironmentConfig) -> Self {
        self.env.config = config;
        self
    }

    // ===== 実行メソッド =====

    /// 指定ステップ数だけ実行し、結果を返す
    ///
    /// `budget` 回の式評価を実行する。途中で完了/エラーに到達した場合は即座に返す。
    /// `budget` を消費しきった場合は `Suspended` を返す。
    pub fn step(&mut self, budget: usize) -> StepResult {
        if self.completed {
            return StepResult::Complete {
                return_value: self.return_value,
            };
        }

        for _ in 0..budget {
            match self.execute_one_step() {
                ExecuteResult::Continue => {}
                ExecuteResult::Complete(value) => {
                    self.completed = true;
                    self.return_value = value;
                    return StepResult::Complete {
                        return_value: value,
                    };
                }
                ExecuteResult::Error(e) => {
                    return StepResult::Error(e);
                }
            }
        }

        StepResult::Suspended
    }

    /// 完了まで一括実行（最大ステップ制限付き）
    ///
    /// `max_steps` ステップまでに完了しない場合は `Suspended` を返す。
    pub fn run(&mut self, max_steps: usize) -> StepResult {
        self.step(max_steps)
    }

    // ===== 状態参照メソッド =====

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool {
        self.completed
    }

    /// 総式評価回数
    pub fn total_steps(&self) -> usize {
        self.total_steps
    }

    /// stdout の内容を文字列として取得する（テスト用）
    ///
    /// `with_io()` で外部 stdout を指定した場合は空文字列を返す。
    pub fn get_stdout_string(&self) -> String {
        match &self.stdout_capture {
            Some(buf) => String::from_utf8_lossy(&buf.borrow()).to_string(),
            None => String::new(),
        }
    }

    /// 戻り値を取得する（完了時のみ有効）
    pub fn return_value(&self) -> Option<i64> {
        self.return_value
    }

    /// トレース結果への参照を返す
    pub fn traced(&self) -> &BTreeMap<i64, i64> {
        &self.traced
    }

    /// stdout をフラッシュする
    pub fn flush(&mut self) {
        self.env.flush();
    }

    // ===== プライベートメソッド =====

    /// 1ステップ（1式評価）の実行
    ///
    /// フレームスタックの末尾を見て、対応する処理を実行する。
    fn execute_one_step(&mut self) -> ExecuteResult {
        if self.flow.is_some() {
            let result = self.propagate_flow();
            match result {
                ExecuteResult::Complete(_) | ExecuteResult::Error(_) => return result,
                ExecuteResult::Continue => {
                    if self.flow.is_none() {
                        return result; // フロー完全処理済み
                    }
                    // flow がまだセットされている場合、ループフレームが処理する
                    // （propagate_flow は WhileLoop/ForLoop で Break/Continue を止める）
                }
            }
        }
        if self.frames.is_empty() {
            return ExecuteResult::Complete(None);
        }
        match self.frames.last() {
            Some(Frame::GlobalInit { .. }) => self.step_global_init(),
            Some(Frame::ExecBlock  { .. }) => self.step_exec_block(),
            Some(Frame::EvalExpr   { .. }) => self.step_eval_expr(),
            Some(Frame::WhileLoop  { .. }) => self.step_while(),
            Some(Frame::ForLoop    { .. }) => self.step_for(),
            None => ExecuteResult::Complete(None),
        }
    }

    fn propagate_flow(&mut self) -> ExecuteResult {
        let flow = match self.flow.clone() {
            Some(f) => f,
            None => return ExecuteResult::Continue,
        };
        loop {
            match self.frames.last() {
                None => {
                    let val = if let FlowControl::Return(v) = flow { Some(v) } else { None };
                    self.flow = None;
                    return ExecuteResult::Complete(val);
                }
                Some(frame) => {
                    // WhileLoop/ForLoop は Break/Continue を step_while/step_for の WaitBody で処理
                    let loop_handles = match (frame, &flow) {
                        (Frame::WhileLoop { .. }, FlowControl::Break | FlowControl::Continue) => true,
                        (Frame::ForLoop   { .. }, FlowControl::Break | FlowControl::Continue) => true,
                        _ => false,
                    };
                    if loop_handles { return ExecuteResult::Continue; }

                    let frame = self.frames.pop().unwrap();
                    match frame {
                        Frame::ExecBlock { completion: BlockCompletion::MainFunc { func_idx, scope_addr }, .. } => {
                            if let FlowControl::Return(val) = &flow {
                                let v = *val;
                                self.save_static_vars(func_idx, scope_addr);
                                self.leave_scope(scope_addr);
                                self.flow = None;
                                return ExecuteResult::Complete(Some(v));
                            }
                            self.save_static_vars(func_idx, scope_addr);
                            self.leave_scope(scope_addr);
                        }
                        Frame::ExecBlock { completion: BlockCompletion::UserFunc { func_idx, scope_addr }, .. } => {
                            if let FlowControl::Return(val) = &flow {
                                let v = *val;
                                self.save_static_vars(func_idx, scope_addr);
                                self.leave_scope(scope_addr);
                                self.value_stack.push(v);
                                self.flow = None;
                                return ExecuteResult::Continue;
                            }
                            self.save_static_vars(func_idx, scope_addr);
                            self.leave_scope(scope_addr);
                        }
                        Frame::ExecBlock { completion: BlockCompletion::ScopeBlock { scope_addr, .. }, .. } => {
                            self.leave_scope(scope_addr);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // ─── GlobalInit ───

    fn step_global_init(&mut self) -> ExecuteResult {
        let phase = match self.frames.last_mut() {
            Some(Frame::GlobalInit { phase }) => std::mem::replace(phase, GlobalInitPhase::CallMain),
            _ => unreachable!(),
        };
        match phase {
            GlobalInitPhase::AllocGlobals => {
                self.env.global_base_addr = self.env.allocator.alloc_internal_uninit(
                    self.scope.variable_count, self.env.config.randomize_uninit);
                for func_idx in 0..self.scope.functions.len() {
                    let has_static = self.scope.functions[func_idx].block.scope.variables
                        .iter().any(|v| v.is_static);
                    if has_static {
                        let sa = self.env.allocator.alloc_internal_uninit(
                            self.scope.functions[func_idx].block.scope.variable_count,
                            self.env.config.randomize_uninit);
                        self.env.function_static_addrs.insert(func_idx, sa);
                    }
                }
                // 静的初期化ステートメントなければスキップ
                if self.scope.static_init_statements.is_empty() {
                    self.set_global_phase(GlobalInitPhase::ExecFuncStaticInits { func_idx: 0 });
                } else {
                    self.set_global_phase(GlobalInitPhase::ExecRootStaticInit { stmt_idx: 0 });
                }
                ExecuteResult::Continue
            }
            GlobalInitPhase::ExecRootStaticInit { stmt_idx } => {
                if stmt_idx >= self.scope.static_init_statements.len() {
                    self.set_global_phase(GlobalInitPhase::ExecFuncStaticInits { func_idx: 0 });
                    return ExecuteResult::Continue;
                }
                let ptr: StmtsPtr = &self.scope.static_init_statements as *const _;
                // set_global_phase は frames.push より先に呼ぶ（last_mut が GlobalInit を指すように）
                self.set_global_phase(GlobalInitPhase::ExecFuncStaticInits { func_idx: 0 });
                self.frames.push(Frame::ExecBlock {
                    stmts: ptr, next_idx: stmt_idx, last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::GlobalStmts,
                });
                ExecuteResult::Continue
            }
            GlobalInitPhase::ExecFuncStaticInits { func_idx } => {
                let len = self.scope.functions.len();
                let mut fi = func_idx;
                while fi < len {
                    let f = &self.scope.functions[fi];
                    if f.block.scope.variables.iter().any(|v| v.is_static)
                        && !f.block.scope.static_init_statements.is_empty() { break; }
                    fi += 1;
                }
                if fi >= len {
                    if self.scope.root_statements.is_empty() {
                        self.set_global_phase(GlobalInitPhase::CallMain);
                    } else {
                        self.set_global_phase(GlobalInitPhase::ExecRootStmts { stmt_idx: 0 });
                    }
                    return ExecuteResult::Continue;
                }
                let static_addr = *self.env.function_static_addrs.get(&fi).unwrap();
                self.set_global_phase(GlobalInitPhase::ExecFuncStaticStmt {
                    func_idx: fi, static_addr, stmt_idx: 0 });
                ExecuteResult::Continue
            }
            GlobalInitPhase::ExecFuncStaticStmt { func_idx, static_addr, stmt_idx } => {
                let stmts = &self.scope.functions[func_idx].block.scope.static_init_statements;
                if stmt_idx >= stmts.len() {
                    self.set_global_phase(GlobalInitPhase::ExecFuncStaticInits { func_idx: func_idx + 1 });
                    return ExecuteResult::Continue;
                }
                let ptr: StmtsPtr = stmts as *const _;
                // set_global_phase は frames.push より先に呼ぶ（last_mut が GlobalInit を指すように）
                self.set_global_phase(GlobalInitPhase::ExecFuncStaticInits { func_idx: func_idx + 1 });
                self.scope_stack.push(static_addr);
                self.frames.push(Frame::ExecBlock {
                    stmts: ptr, next_idx: stmt_idx, last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::GlobalStmts,
                });
                ExecuteResult::Continue
            }
            GlobalInitPhase::ExecRootStmts { stmt_idx } => {
                if stmt_idx >= self.scope.root_statements.len() {
                    self.set_global_phase(GlobalInitPhase::CallMain);
                    return ExecuteResult::Continue;
                }
                let ptr: StmtsPtr = &self.scope.root_statements as *const _;
                // set_global_phase は frames.push より先に呼ぶ（last_mut が GlobalInit を指すように）
                self.set_global_phase(GlobalInitPhase::CallMain);
                self.frames.push(Frame::ExecBlock {
                    stmts: ptr, next_idx: stmt_idx, last_value: 0,
                    waiting: ExecBlockWait::None,
                    completion: BlockCompletion::GlobalStmts,
                });
                ExecuteResult::Continue
            }
            GlobalInitPhase::CallMain => {
                let main_idx = match self.scope.main_function_index {
                    Some(i) => i,
                    None => return ExecuteResult::Error(
                        InterpretError::FunctionNotFound("__main".to_string())),
                };
                self.frames.pop(); // GlobalInit pop
                self.push_func_frame(main_idx, &[], true);
                ExecuteResult::Continue
            }
        }
    }

    fn set_global_phase(&mut self, phase: GlobalInitPhase) {
        if let Some(Frame::GlobalInit { phase: p }) = self.frames.last_mut() { *p = phase; }
    }

    fn push_func_frame(&mut self, func_idx: usize, args: &[i64], is_main: bool) {
        let has_static = self.scope.functions[func_idx].block.scope.variables
            .iter().any(|v| v.is_static);
        let scope_addr = self.env.allocator.alloc_internal_uninit(
            self.scope.functions[func_idx].block.scope.variable_count,
            self.env.config.randomize_uninit);
        if has_static { self.load_static_vars(func_idx, scope_addr); }
        let arg_indices: Vec<usize> = self.scope.functions[func_idx].arg_indices.clone();
        for (i, &v) in args.iter().enumerate() {
            if i < arg_indices.len() {
                self.env.allocator.set(scope_addr + arg_indices[i] as i64, v);
            }
        }
        self.scope_stack.push(scope_addr);
        let stmts: StmtsPtr = &self.scope.functions[func_idx].block.statements as *const _;
        let completion = if is_main {
            BlockCompletion::MainFunc { func_idx, scope_addr }
        } else {
            BlockCompletion::UserFunc { func_idx, scope_addr }
        };
        self.frames.push(Frame::ExecBlock {
            stmts, next_idx: 0, last_value: 0,
            waiting: ExecBlockWait::None, completion,
        });
    }

    // ─── 組み込み関数 ───

    fn exec_builtin(&mut self, kind: BuiltinFunctionKind, args: &[i64]) -> i64 {
        let a0 = args.first().copied().unwrap_or(0);
        match kind {
            BuiltinFunctionKind::Puti  => { self.env.write_int(a0);  a0 }
            BuiltinFunctionKind::Putc  => { self.env.write_char(a0); a0 }
            BuiltinFunctionKind::Geti  => self.env.read_int(),
            BuiltinFunctionKind::Getc  => self.env.read_char(),
            BuiltinFunctionKind::Clog  => {
                if !self.env.config.ignore_debug { println!("__clog: {}", a0); }
                a0
            }
            BuiltinFunctionKind::Assert => {
                if !self.env.config.ignore_debug && a0 == 0 { panic!("assertion failed"); }
                a0
            }
            BuiltinFunctionKind::AssertNot => {
                if !self.env.config.ignore_debug && a0 != 0 {
                    panic!("assertion failed: {} != 0", a0);
                }
                a0
            }
            BuiltinFunctionKind::Trace => {
                if !self.env.config.ignore_debug {
                    *self.traced.entry(a0).or_insert(0) += 1;
                }
                0
            }
            BuiltinFunctionKind::Alloc => self.env.allocator.alloc(a0),
            BuiltinFunctionKind::Free  => { self.env.allocator.free(a0); 0 }
        }
    }

    fn exec_internal_builtin(&mut self, kind: &InternalBuiltinFunctionKind) -> i64 {
        match kind {
            InternalBuiltinFunctionKind::Getiv(v) => {
                let val = self.env.read_int();
                self.set_variable(v, val);
                val
            }
            InternalBuiltinFunctionKind::Getcv(v) => {
                let val = self.env.read_char();
                self.set_variable(v, val);
                val
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
