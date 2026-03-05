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

type StmtsPtr = *const Vec<LocatedExecStatement>;
type ExprPtr = *const LocatedExecExpression;
type ArgsPtr = *const Vec<Box<LocatedExecExpression>>;
type BlockPtr = *const Block;

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

/// 制御フローの種別
#[derive(Clone, Debug)]
enum FlowControl {
    Return(i64),
    Break,
    Continue,
}

/// グローバル初期化のフェーズ
enum GlobalInitPhase {
    AllocGlobals,
    ExecRootStaticInit { stmt_idx: usize },
    ExecFuncStaticInits { func_idx: usize },
    ExecFuncStaticStmt { func_idx: usize, static_addr: i64, stmt_idx: usize },
    ExecRootStmts { stmt_idx: usize },
    CallMain,
}

/// ExecBlock 完了時のアクション
enum BlockCompletion {
    MainFunc  { func_idx: usize, scope_addr: i64 },
    UserFunc  { func_idx: usize, scope_addr: i64 },
    ScopeBlock { scope_addr: i64, push_value: bool },
    GlobalStmts,
}

/// ExecBlock のサブフレーム待機状態
#[derive(Clone, Debug, PartialEq, Eq)]
enum ExecBlockWait {
    None,
    WaitExpr,
    WaitStmt,
    WaitReturn,
}

/// 式評価の継続状態
enum EvalCont {
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

/// while ループのフェーズ
enum WhilePhase {
    EvalCond,
    CheckCond,
    WaitBody,
}

#[allow(dead_code)]

/// for ループのフェーズ
enum ForPhase {
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
enum Frame {
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

/// `execute_one_step` の戻り値（VM 内部使用）
enum ExecuteResult {
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
    scope:          Scope,
    frames:         Vec<Frame>,
    value_stack:    Vec<i64>,
    scope_stack:    Vec<i64>,
    flow:           Option<FlowControl>,
    env:            Environment,
    stdout_capture: Option<Rc<RefCell<Vec<u8>>>>,
    total_steps:    usize,
    pub traced:     BTreeMap<i64, i64>,
    completed:      bool,
    return_value:   Option<i64>,
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

    // ─── 変数アクセス ───

    fn resolve_addr(&self, id: &IdentifierRef) -> i64 {
        if id.is_global {
            self.env.global_base_addr + id.local_index as i64
        } else {
            let depth = id.scope_depth;
            let idx = self.scope_stack.len().saturating_sub(1 + depth);
            self.scope_stack[idx] + id.local_index as i64
        }
    }

    fn get_variable(&self, id: &IdentifierRef) -> i64 {
        self.env.allocator.get(self.resolve_addr(id))
    }

    fn set_variable(&mut self, id: &IdentifierRef, v: i64) {
        let addr = self.resolve_addr(id);
        self.env.allocator.set(addr, v);
    }

    fn enter_block(&mut self, scope: &crate::semantic_analyzer::Scope) -> i64 {
        let base = self.env.allocator.alloc_internal_uninit(
            scope.variable_count, self.env.config.randomize_uninit);
        self.scope_stack.push(base);
        base
    }

    fn leave_scope(&mut self, scope_addr: i64) {
        if self.scope_stack.last() == Some(&scope_addr) {
            self.scope_stack.pop();
        }
        self.env.allocator.free_internal(scope_addr);
    }

    // ─── static 変数 ───

    fn save_static_vars(&mut self, func_idx: usize, scope_addr: i64) {
        if let Some(&static_addr) = self.env.function_static_addrs.get(&func_idx) {
            let vars: Vec<_> = self.scope.functions[func_idx].block.scope.variables
                .iter().filter(|v| v.is_static)
                .map(|v| (v.slot_index, v.array_size.unwrap_or(1))).collect();
            for (slot, count) in vars {
                for i in 0..count {
                    let v = self.env.allocator.get(scope_addr + (slot + i) as i64);
                    self.env.allocator.set(static_addr + (slot + i) as i64, v);
                }
            }
        }
    }

    fn load_static_vars(&mut self, func_idx: usize, scope_addr: i64) {
        if let Some(&static_addr) = self.env.function_static_addrs.get(&func_idx) {
            let vars: Vec<_> = self.scope.functions[func_idx].block.scope.variables
                .iter().filter(|v| v.is_static)
                .map(|v| (v.slot_index, v.array_size.unwrap_or(1))).collect();
            for (slot, count) in vars {
                for i in 0..count {
                    let v = self.env.allocator.get(static_addr + (slot + i) as i64);
                    self.env.allocator.set(scope_addr + (slot + i) as i64, v);
                }
            }
        }
    }

    // ─── ExecBlock ───

    fn step_exec_block(&mut self) -> ExecuteResult {
        let (stmts_ptr, next_idx, last_value, waiting) = match self.frames.last() {
            Some(Frame::ExecBlock { stmts, next_idx, last_value, waiting, .. }) =>
                (*stmts, *next_idx, *last_value, waiting.clone()),
            _ => unreachable!(),
        };

        match waiting {
            ExecBlockWait::WaitExpr => {
                let val = self.value_stack.pop().unwrap_or(0);
                if let Some(Frame::ExecBlock { last_value: lv, waiting: w, .. }) = self.frames.last_mut() {
                    *lv = val; *w = ExecBlockWait::None;
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

        if let Some(Frame::ExecBlock { next_idx: ni, .. }) = self.frames.last_mut() { *ni += 1; }

        let stmt = &stmts[next_idx].statement;
        match stmt {
            ExecStatement::Expression(expr) => {
                let ep: ExprPtr = expr.as_ref() as *const _;
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::WaitExpr;
                }
                self.frames.push(Frame::EvalExpr { expr: ep, cont: EvalCont::Start });
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
                self.frames.push(Frame::EvalExpr { expr: ep, cont: EvalCont::Start });
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
                let cp: ExprPtr  = cond.as_ref() as *const _;
                let bp: BlockPtr = body as *const _;
                let m = *mode;
                if let Some(Frame::ExecBlock { waiting: w, .. }) = self.frames.last_mut() {
                    *w = ExecBlockWait::WaitStmt;
                }
                self.frames.push(Frame::WhileLoop {
                    mode: m, cond: cp, body: bp, phase: WhilePhase::EvalCond,
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
                    mode: m, init_block: ib, cond_block: cb, step_block: sb, body_block: bb,
                    phase: ForPhase::StartInit,
                });
                ExecuteResult::Continue
            }
        }
    }

    fn finish_exec_block(&mut self, last_value: i64) -> ExecuteResult {
        let frame = self.frames.pop().unwrap();
        match frame {
            Frame::ExecBlock { completion: BlockCompletion::MainFunc { func_idx, scope_addr }, .. } => {
                self.save_static_vars(func_idx, scope_addr);
                self.leave_scope(scope_addr);
                ExecuteResult::Complete(Some(last_value))
            }
            Frame::ExecBlock { completion: BlockCompletion::UserFunc { func_idx, scope_addr }, .. } => {
                self.save_static_vars(func_idx, scope_addr);
                self.leave_scope(scope_addr);
                self.value_stack.push(last_value);
                ExecuteResult::Continue
            }
            Frame::ExecBlock { completion: BlockCompletion::ScopeBlock { scope_addr, push_value }, .. } => {
                self.leave_scope(scope_addr);
                if push_value { self.value_stack.push(last_value); }
                ExecuteResult::Continue
            }
            Frame::ExecBlock { completion: BlockCompletion::GlobalStmts, .. } => {
                ExecuteResult::Continue
            }
            _ => ExecuteResult::Continue,
        }
    }

    // ─── EvalExpr ───

    fn step_eval_expr(&mut self) -> ExecuteResult {
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

    fn eval_start(&mut self, expr: &LocatedExecExpression) -> ExecuteResult {
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

    fn push_assign(&mut self, lhs: &LocatedExecExpression, rhs: &LocatedExecExpression) -> ExecuteResult {
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

    fn finish_eval(&mut self, value: i64) -> ExecuteResult {
        self.frames.pop();
        self.value_stack.push(value);
        ExecuteResult::Continue
    }

    fn set_eval_cont(&mut self, cont: EvalCont) {
        if let Some(Frame::EvalExpr { cont: c, .. }) = self.frames.last_mut() { *c = cont; }
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

    // ─── WhileLoop ───

    fn step_while(&mut self) -> ExecuteResult {
        let (mode, cond, body, phase) = match self.frames.last_mut() {
            Some(Frame::WhileLoop { mode, cond, body, phase }) => {
                (*mode, *cond, *body, std::mem::replace(phase, WhilePhase::EvalCond))
            }
            _ => unreachable!(),
        };
        match phase {
            WhilePhase::EvalCond => {
                self.set_while_phase(WhilePhase::CheckCond);
                self.frames.push(Frame::EvalExpr { expr: cond, cont: EvalCont::Start });
                ExecuteResult::Continue
            }
            WhilePhase::CheckCond => {
                let cv = self.value_stack.pop().unwrap_or(0);
                let ok = match mode {
                    ConditionMode::NonZero  => cv != 0,
                    ConditionMode::Zero     => cv == 0,
                    ConditionMode::Negative => cv < 0,
                };
                if !ok { self.frames.pop(); return ExecuteResult::Continue; }
                let block = unsafe { &*body };
                let sa = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.set_while_phase(WhilePhase::WaitBody);
                self.frames.push(Frame::ExecBlock {
                    stmts, next_idx: 0, last_value: 0, waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock { scope_addr: sa, push_value: false },
                });
                ExecuteResult::Continue
            }
            WhilePhase::WaitBody => {
                if let Some(FlowControl::Break) = &self.flow {
                    self.flow = None;
                    self.frames.pop();
                    return ExecuteResult::Continue;
                }
                if let Some(FlowControl::Continue) = &self.flow { self.flow = None; }
                self.set_while_phase(WhilePhase::EvalCond);
                ExecuteResult::Continue
            }
        }
    }

    fn set_while_phase(&mut self, p: WhilePhase) {
        if let Some(Frame::WhileLoop { phase, .. }) = self.frames.last_mut() { *phase = p; }
    }

    // ─── ForLoop ───

    fn step_for(&mut self) -> ExecuteResult {
        let (mode, ib, cb, sb, bb, phase) = match self.frames.last_mut() {
            Some(Frame::ForLoop { mode, init_block, cond_block, step_block, body_block, phase }) => {
                (*mode, *init_block, *cond_block, *step_block, *body_block,
                 std::mem::replace(phase, ForPhase::StartInit))
            }
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
                self.set_for_phase(ForPhase::WaitInit { init_scope_addr: sa });
                self.frames.push(Frame::ExecBlock {
                    stmts, next_idx: 0, last_value: 0, waiting: ExecBlockWait::None,
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
                    stmts, next_idx: 0, last_value: 0, waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock { scope_addr: sa, push_value: true },
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
                    ConditionMode::NonZero  => cv != 0,
                    ConditionMode::Zero     => cv == 0,
                    ConditionMode::Negative => cv < 0,
                };
                if !ok {
                    if self.scope_stack.last() == Some(&init_scope_addr) { self.scope_stack.pop(); }
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
                    stmts, next_idx: 0, last_value: 0, waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock { scope_addr: sa, push_value: false },
                });
                ExecuteResult::Continue
            }
            ForPhase::WaitBody { init_scope_addr } => {
                if let Some(FlowControl::Break) = &self.flow {
                    self.flow = None;
                    if self.scope_stack.last() == Some(&init_scope_addr) { self.scope_stack.pop(); }
                    self.env.allocator.free_internal(init_scope_addr);
                    self.frames.pop();
                    return ExecuteResult::Continue;
                }
                if let Some(FlowControl::Continue) = &self.flow { self.flow = None; }
                self.set_for_phase(ForPhase::StartStep { init_scope_addr });
                ExecuteResult::Continue
            }
            ForPhase::StartStep { init_scope_addr } => {
                let block = unsafe { &*sb };
                let sa = self.enter_block(&block.scope);
                let stmts: StmtsPtr = &block.statements as *const _;
                self.set_for_phase(ForPhase::WaitStep { init_scope_addr });
                self.frames.push(Frame::ExecBlock {
                    stmts, next_idx: 0, last_value: 0, waiting: ExecBlockWait::None,
                    completion: BlockCompletion::ScopeBlock { scope_addr: sa, push_value: false },
                });
                ExecuteResult::Continue
            }
            ForPhase::WaitStep { init_scope_addr } => {
                if let Some(FlowControl::Break) = &self.flow {
                    self.flow = None;
                    if self.scope_stack.last() == Some(&init_scope_addr) { self.scope_stack.pop(); }
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
        if let Some(Frame::ForLoop { phase, .. }) = self.frames.last_mut() { *phase = p; }
    }
}

// ===== tests =====

#[cfg(test)]
mod tests {
    use super::*;

    fn run_src(src: &str) -> (Option<i64>, String) {
        let mut vm = NospaceVM::from_source(src).expect("parse/analyze failed");
        match vm.run(1_000_000) {
            StepResult::Complete { return_value } => (return_value, vm.get_stdout_string()),
            StepResult::Error(e) => panic!("runtime error: {:?}", e),
            StepResult::Suspended => panic!("did not complete within budget"),
        }
    }

    #[test]
    fn test_from_source_parse_error() {
        assert!(NospaceVM::from_source("this is not valid nospace!!!!").is_err());
    }

    #[test]
    fn test_simple_return() {
        let (rv, _) = run_src("func: __main() { return: 42; }");
        assert_eq!(rv, Some(42));
    }

    #[test]
    fn test_puti_output() {
        let (_, out) = run_src("func: __main() { __puti(123); }");
        assert_eq!(out, "123");
    }

    #[test]
    fn test_arithmetic() {
        let (rv, _) = run_src("func: __main() { return: 3 + 4 * 2; }");
        assert_eq!(rv, Some(11));
    }

    #[test]
    fn test_variable_assign() {
        let (rv, _) = run_src("func: __main() { let: x; x = 10; return: x + 5; }");
        assert_eq!(rv, Some(15));
    }

    #[test]
    fn test_if_true() {
        let (rv, _) = run_src(
            "func: __main() { let: x; x = if: 1 { 10; } else: { 20; }; return: x; }");
        assert_eq!(rv, Some(10));
    }

    #[test]
    fn test_if_false() {
        let (rv, _) = run_src(
            "func: __main() { let: x; x = if: 0 { 20; } else: { 10; }; return: x; }");
        assert_eq!(rv, Some(10));
    }

    #[test]
    fn test_while_loop() {
        let (rv, _) = run_src(
            "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 5 { s = s + i; i = i + 1; }; return: s; }");
        assert_eq!(rv, Some(10));
    }

    #[test]
    fn test_function_call() {
        let (rv, _) = run_src(
            "func: double(x) { return: x * 2; } func: __main() { return: double(21); }");
        assert_eq!(rv, Some(42));
    }

    #[test]
    fn test_recursive_function() {
        // まず fib(2) = 1 を確認
        let (rv2, _) = run_src(r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(2); }"#);
        assert_eq!(rv2, Some(1), "fib(2) should be 1");

        let (rv, _) = run_src(r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(10); }"#);
        assert_eq!(rv, Some(55));
    }

    #[test]
    fn test_step_suspension() {
        let src = "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 100 { s = s + i; i = i + 1; }; return: s; }";
        let mut vm = NospaceVM::from_source(src).unwrap();
        let r1 = vm.step(5);
        assert!(matches!(r1, StepResult::Suspended), "expected Suspended, got {:?}", r1);
        let r2 = vm.run(10_000_000);
        assert!(matches!(r2, StepResult::Complete { return_value: Some(4950) }),
            "expected Complete(4950), got {:?}", r2);
    }

    #[test]
    fn test_complete_is_idempotent() {
        let mut vm = NospaceVM::from_source("func: __main() { return: 1; }").unwrap();
        let r1 = vm.run(1_000_000);
        assert!(matches!(r1, StepResult::Complete { return_value: Some(1) }));
        let r2 = vm.step(1);
        assert!(matches!(r2, StepResult::Complete { return_value: Some(1) }));
    }

    #[test]
    fn test_initial_state() {
        let vm = NospaceVM::from_source("func: __main() { return: 42; }").unwrap();
        assert!(!vm.is_complete());
        assert_eq!(vm.total_steps(), 0);
        assert_eq!(vm.return_value(), None);
    }

    #[test]
    fn test_builder_with_stdin() {
        let stdin: Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new("hello".as_bytes())));
        let vm = NospaceVM::from_source("func: __main() { return: 0; }").unwrap().with_stdin(stdin);
        assert!(!vm.is_complete());
    }

    #[test]
    fn test_builder_with_config() {
        let vm = NospaceVM::from_source("func: __main() { return: 0; }")
            .unwrap().with_config(EnvironmentConfig::new());
        assert!(!vm.is_complete());
    }

    #[test]
    fn test_with_io_disables_capture() {
        let stdin:  Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new(b"" as &[u8])));
        let stdout: Box<dyn Write>   = Box::new(Vec::<u8>::new());
        let vm = NospaceVM::from_source("func: __main() { return: 0; }")
            .unwrap().with_io(stdin, stdout);
        assert_eq!(vm.get_stdout_string(), "");
    }

    #[test]
    fn test_step_result_debug() {
        let _ = format!("{:?}", StepResult::Suspended);
        let _ = format!("{:?}", StepResult::Complete { return_value: Some(1) });
        let _ = format!("{:?}", StepResult::Error(InterpretError::FunctionNotFound("f".into())));
    }

    #[test]
    fn test_get_stdout_string_initially_empty() {
        let vm = NospaceVM::from_source("func: __main() { return: 0; }").unwrap();
        assert_eq!(vm.get_stdout_string(), "");
    }

    // ===== Phase 3: step(1) 中断・再開テスト =====

    #[test]
    fn test_step_one_at_a_time() {
        // step(1) を繰り返し呼んですべて実行完了できることを確認
        let src = "func: __main() { return: 1 + 2 + 3; }";
        let mut vm = NospaceVM::from_source(src).unwrap();
        let mut step_calls = 0;
        loop {
            match vm.step(1) {
                StepResult::Complete { return_value } => {
                    assert_eq!(return_value, Some(6));
                    break;
                }
                StepResult::Suspended => {
                    step_calls += 1;
                    assert!(step_calls < 1000, "step(1) loop exceeded 1000 iterations");
                }
                StepResult::Error(e) => panic!("unexpected error: {:?}", e),
            }
        }
        assert!(step_calls > 0, "should have suspended at least once");
        // total_steps は式評価回数のみカウント（GlobalInit 等はカウントしない）
        assert!(vm.total_steps() > 0, "total_steps should be > 0");
    }

    #[test]
    fn test_step_one_with_function_call() {
        // 関数呼び出しを含む場合も step(1) で正しく実行できること
        let src = r#"
func: add(a, b) { return: a + b; }
func: __main() { return: add(10, 20); }
"#;
        let mut vm = NospaceVM::from_source(src).unwrap();
        let mut steps = 0;
        loop {
            match vm.step(1) {
                StepResult::Complete { return_value } => {
                    assert_eq!(return_value, Some(30));
                    break;
                }
                StepResult::Suspended => {
                    steps += 1;
                    assert!(steps < 10000, "step(1) loop exceeded limit");
                }
                StepResult::Error(e) => panic!("unexpected error: {:?}", e),
            }
        }
    }

    #[test]
    fn test_step_one_with_loop() {
        // ループを含むプログラムを step(1) で実行
        let src = "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 10 { s = s + i; i = i + 1; }; return: s; }";
        let mut vm = NospaceVM::from_source(src).unwrap();
        let mut steps = 0;
        loop {
            match vm.step(1) {
                StepResult::Complete { return_value } => {
                    assert_eq!(return_value, Some(45));
                    break;
                }
                StepResult::Suspended => {
                    steps += 1;
                    assert!(steps < 100000, "step(1) loop exceeded limit");
                }
                StepResult::Error(e) => panic!("unexpected error: {:?}", e),
            }
        }
    }

    #[test]
    fn test_step_one_with_recursion() {
        // 再帰を含むプログラムを step(1) で実行
        let src = r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(6); }
"#;
        let mut vm = NospaceVM::from_source(src).unwrap();
        let mut steps = 0;
        loop {
            match vm.step(1) {
                StepResult::Complete { return_value } => {
                    assert_eq!(return_value, Some(8));
                    break;
                }
                StepResult::Suspended => {
                    steps += 1;
                    assert!(steps < 1_000_000, "step(1) loop exceeded limit");
                }
                StepResult::Error(e) => panic!("unexpected error: {:?}", e),
            }
        }
    }

    #[test]
    fn test_step_one_preserves_state() {
        // step(1) の合間で状態が正しく保存されることを確認
        let src = "func: __main() { __puti(1); __puti(2); __puti(3); return: 0; }";
        let mut vm = NospaceVM::from_source(src).unwrap();

        // 途中まで実行
        let mut suspended_count = 0;
        for _ in 0..3 {
            match vm.step(1) {
                StepResult::Suspended => { suspended_count += 1; }
                StepResult::Complete { .. } => break,
                StepResult::Error(e) => panic!("unexpected error: {:?}", e),
            }
        }

        // 残りを実行
        let result = vm.run(1_000_000);
        match result {
            StepResult::Complete { .. } => {}
            _ => panic!("expected completion"),
        }
        assert_eq!(vm.get_stdout_string(), "123");
    }

    // ===== Phase 3: max_expression_count 相当（Suspended + 再開） =====

    #[test]
    fn test_suspension_and_resume() {
        // 少ない budget で中断し、追加の budget で完了できることを確認
        let src = "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 100 { s = s + i; i = i + 1; }; return: s; }";
        let mut vm = NospaceVM::from_source(src).unwrap();

        // 少ない budget → Suspended
        let r1 = vm.step(10);
        assert!(matches!(r1, StepResult::Suspended), "expected Suspended, got {:?}", r1);
        assert!(!vm.is_complete());
        let steps_after_first = vm.total_steps();
        assert!(steps_after_first > 0);

        // 追加の budget → まだ Suspended かもしれない
        let r2 = vm.step(10);
        assert!(!matches!(r2, StepResult::Error(_)));
        let steps_after_second = vm.total_steps();
        assert!(steps_after_second > steps_after_first);

        // 十分な budget で完了
        let r3 = vm.run(1_000_000);
        assert!(matches!(r3, StepResult::Complete { return_value: Some(4950) }),
            "expected Complete(4950), got {:?}", r3);
        assert!(vm.is_complete());
    }

    #[test]
    fn test_budget_zero_returns_suspended() {
        let mut vm = NospaceVM::from_source("func: __main() { return: 42; }").unwrap();
        let r = vm.step(0);
        assert!(matches!(r, StepResult::Suspended));
        assert!(!vm.is_complete());
    }

    #[test]
    fn test_total_steps_increments_correctly() {
        // total_steps が式評価回数と一致することを確認
        let src = "func: __main() { return: 1 + 2; }";
        let mut vm = NospaceVM::from_source(src).unwrap();
        assert_eq!(vm.total_steps(), 0);

        vm.run(1_000_000);
        assert!(vm.total_steps() > 0, "total_steps should be > 0 after execution");
    }

    #[test]
    fn test_repeated_suspension_accumulates_steps() {
        let src = "func: __main() { let: i; i = 0; while: i < 50 { i = i + 1; }; return: i; }";
        let mut vm = NospaceVM::from_source(src).unwrap();

        // 十分な反復で完了まで実行
        let mut suspend_count = 0;
        loop {
            match vm.step(20) {
                StepResult::Suspended => {
                    suspend_count += 1;
                    assert!(suspend_count < 10000, "too many suspensions");
                }
                StepResult::Complete { return_value } => {
                    assert_eq!(return_value, Some(50));
                    break;
                }
                StepResult::Error(e) => panic!("unexpected error: {:?}", e),
            }
        }
        assert!(suspend_count > 0, "should have suspended at least once");
        assert!(vm.total_steps() > 0, "total_steps should increase across run");
    }

    // ===== Phase 3: 再帰版インタプリタとの結果一致テスト =====

    /// 再帰版インタプリタと NospaceVM の結果を比較するヘルパー
    fn assert_vm_matches_interpreter(src: &str) {
        use crate::interpreter;
        use super::Environment;

        // 再帰版インタプリタで実行
        let tokens = crate::token_parser::parse_to_tokens(&src.to_string()).unwrap();
        let tree = crate::tree_parser::parse_to_tree(&tokens).unwrap();
        let scope = crate::semantic_analyzer::analyze(&tree).unwrap();

        let mut env = Environment::new();
        interpreter::interpret_global(&mut env, &scope).expect("global init failed");
        let interp_result = interpreter::interpret_func(&mut env, &scope, "__main");
        let interp_traced = env.traced.clone();

        // NospaceVM で実行
        let tokens2 = crate::token_parser::parse_to_tokens(&src.to_string()).unwrap();
        let tree2 = crate::tree_parser::parse_to_tree(&tokens2).unwrap();
        let scope2 = crate::semantic_analyzer::analyze(&tree2).unwrap();

        let mut vm = NospaceVM::from_scope(scope2).expect("failed to create NospaceVM");
        let vm_result = vm.run(10_000_000);

        // 結果を比較
        match (&interp_result, &vm_result) {
            (Ok(interp_rv), StepResult::Complete { return_value: vm_rv }) => {
                assert_eq!(interp_rv, vm_rv,
                    "return value mismatch: interpreter={:?}, vm={:?}", interp_rv, vm_rv);
            }
            (Err(interp_err), StepResult::Error(vm_err)) => {
                // 両方エラー: OK（エラーメッセージの完全一致は不要）
                let _ = (interp_err, vm_err);
            }
            _ => {
                panic!("result type mismatch: interpreter={:?}, vm={:?}", interp_result, vm_result);
            }
        }

        // trace を比較
        assert_eq!(interp_traced, *vm.traced(),
            "trace mismatch:\n  interpreter: {:?}\n  vm: {:?}", interp_traced, vm.traced());
    }

    #[test]
    fn test_match_simple_return() {
        assert_vm_matches_interpreter("func: __main() { return: 42; }");
    }

    #[test]
    fn test_match_arithmetic() {
        assert_vm_matches_interpreter("func: __main() { return: (3 + 5) * 2 - 1; }");
    }

    #[test]
    fn test_match_variables() {
        assert_vm_matches_interpreter(
            "func: __main() { let: x; let: y; x = 10; y = x * 3; return: y - x; }"
        );
    }

    #[test]
    fn test_match_if_expression() {
        assert_vm_matches_interpreter(
            "func: __main() { let: r; r = if: 1 { 100; } else: { 200; }; return: r; }"
        );
    }

    #[test]
    fn test_match_while_loop() {
        assert_vm_matches_interpreter(
            "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 10 { s = s + i; i = i + 1; }; return: s; }"
        );
    }

    #[test]
    fn test_match_function_call() {
        assert_vm_matches_interpreter(r#"
func: square(x) { return: x * x; }
func: __main() { return: square(7); }
"#);
    }

    #[test]
    fn test_match_recursive_function() {
        assert_vm_matches_interpreter(r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(10); }
"#);
    }

    #[test]
    fn test_match_trace() {
        assert_vm_matches_interpreter(r#"
func: __main() {
    __trace(0);
    __trace(0);
    __trace(1);
    return: 0;
}
"#);
    }

    #[test]
    fn test_match_nested_scope() {
        assert_vm_matches_interpreter(r#"
func: __main() {
    let: x;
    x = 1;
    {
        let: y;
        y = 2;
        x = x + y;
    };
    return: x;
}
"#);
    }

    #[test]
    fn test_match_for_loop() {
        assert_vm_matches_interpreter(r#"
func: __main() {
    let: s;
    s = 0;
    for: { let: i(0); } { i < 5; } { i = i + 1; } {
        s = s + i;
    };
    return: s;
}
"#);
    }

    #[test]
    fn test_match_break_continue() {
        assert_vm_matches_interpreter(r#"
func: __main() {
    let: s;
    let: i;
    s = 0;
    i = 0;
    while: i < 20 {
        i = i + 1;
        if: i % 2 == 0 { continue:; };
        if: i > 10 { break:; };
        s = s + i;
    };
    return: s;
}
"#);
    }

    #[test]
    fn test_match_global_variable() {
        assert_vm_matches_interpreter(r#"
let: g;
g = 100;
func: __main() {
    return: g + 1;
}
"#);
    }

    #[test]
    fn test_match_multiple_functions() {
        assert_vm_matches_interpreter(r#"
func: add(a, b) { return: a + b; }
func: mul(a, b) { return: a * b; }
func: __main() { return: add(mul(3, 4), 5); }
"#);
    }
}
