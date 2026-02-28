//! Whitespace インタプリタ
//!
//! Whitespace 命令列を実行するスタックマシン。
//! 全ての実行状態を明示的に保持し、中断・再開可能。

use crate::compiler_ws::instruction::Instruction;
use crate::compiler_ws::types::LabelId;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufRead, Write};
use std::rc::Rc;

// ===== プロファイリング用データ構造 =====

/// 命令別実行カウント（プロファイリング用）
#[derive(Debug, Default, Clone)]
pub struct InstructionCounts {
    pub push: usize,
    pub duplicate: usize,
    pub copy: usize,
    pub swap: usize,
    pub discard: usize,
    pub add: usize,
    pub sub: usize,
    pub mul: usize,
    pub div: usize,
    pub modulo: usize,
    pub store: usize,
    pub retrieve: usize,
    pub label: usize,
    pub call: usize,
    pub jump: usize,
    pub jump_if_zero: usize,
    pub jump_if_negative: usize,
    pub return_count: usize,
    pub exit: usize,
    pub output_char: usize,
    pub output_number: usize,
    pub input_char: usize,
    pub input_number: usize,
}

/// ヒープアクセス統計（プロファイリング用）
#[derive(Debug, Clone, Default)]
pub struct HeapProfileStats {
    /// Store 命令の実行回数
    pub store_count: usize,
    /// Retrieve 命令の実行回数
    pub retrieve_count: usize,
    /// Store したアドレスの (最小値, 最大値)。Store が0回なら None
    pub store_range: Option<(i64, i64)>,
    /// Retrieve したアドレスの (最小値, 最大値)。Retrieve が0回なら None
    pub retrieve_range: Option<(i64, i64)>,
    /// Store または Retrieve でアクセスしたユニークアドレス数
    pub unique_address_count: usize,
    /// ユニークアドレスを追跡する内部セット
    unique_addresses: BTreeSet<i64>,
}

impl HeapProfileStats {
    /// Store アドレスを記録する
    fn record_store(&mut self, addr: i64) {
        self.store_count += 1;
        self.store_range = Some(match self.store_range {
            None => (addr, addr),
            Some((min, max)) => (min.min(addr), max.max(addr)),
        });
        if self.unique_addresses.insert(addr) {
            self.unique_address_count += 1;
        }
    }

    /// Retrieve アドレスを記録する
    fn record_retrieve(&mut self, addr: i64) {
        self.retrieve_count += 1;
        self.retrieve_range = Some(match self.retrieve_range {
            None => (addr, addr),
            Some((min, max)) => (min.min(addr), max.max(addr)),
        });
        if self.unique_addresses.insert(addr) {
            self.unique_address_count += 1;
        }
    }
}

/// スタック深さ統計（プロファイリング用）
#[derive(Debug, Clone, Default)]
pub struct StackProfileStats {
    /// データスタックの最大深さ
    pub max_data_stack_depth: usize,
    /// コールスタックの最大深さ
    pub max_call_stack_depth: usize,
}

/// VM 実行プロファイル統計
#[derive(Debug, Clone, Default)]
pub struct ProfileStats {
    pub instruction_counts: InstructionCounts,
    pub heap: HeapProfileStats,
    pub stack: StackProfileStats,
}

/// 入力待ちの種別
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InputWaitType {
    /// InputChar 命令（1文字入力待ち）
    Char,
    /// InputNumber 命令（数値入力待ち＝1行入力待ち）
    Number,
}

/// VM の実行結果
#[derive(Debug, PartialEq, Eq)]
pub enum StepResult {
    /// 実行継続中（バジェット消費で中断）
    Suspended,
    /// 正常終了（Exit 命令到達）
    Complete,
    /// 実行時エラー
    Error(RuntimeError),
    /// stdin バッファ不足による一時停止
    WaitingForInput(InputWaitType),
}

/// 実行時エラー
#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// スタックアンダーフロー
    StackUnderflow,
    /// ゼロ除算
    DivisionByZero,
    /// 未定義ラベルへのジャンプ
    UndefinedLabel(i64),
    /// ヒープの未初期化アドレスへのアクセス
    UninitializedHeap(i64),
    /// コールスタックアンダーフロー（ret 命令でコールスタックが空）
    CallStackUnderflow,
    /// PC が命令列の範囲外
    ProgramCounterOutOfBounds,
    /// I/O エラー
    IoError(String),
    /// アサーション失敗（拡張 API）
    AssertionFailed(i64),
}

/// read_char / read_number の失敗種別（内部使用）
#[derive(Debug)]
enum ReadResult {
    /// stdin バッファ不足（interactive モード用）
    WouldBlock,
    /// I/O エラー
    IoError(String),
}

/// stdin ソースの種別（内部使用）
enum StdinSource {
    /// 従来の BufRead ベース（非 interactive / テスト用）
    Buffered(Box<dyn BufRead>),
    /// Interactive モード（追記可能バッファ）
    Interactive(InteractiveBuffer),
}

/// 追記可能な stdin バッファ（interactive モード用）
///
/// バッファが空の場合は WouldBlock を返す。
/// close() でストリーム終端を通知すると、その後バッファが空になった時点で EOF を返す。
struct InteractiveBuffer {
    data: std::collections::VecDeque<u8>,
    /// ストリーム終端が通知済みか
    closed: bool,
}

impl InteractiveBuffer {
    fn new() -> Self {
        Self {
            data: std::collections::VecDeque::new(),
            closed: false,
        }
    }

    fn append(&mut self, data: &[u8]) {
        self.data.extend(data);
    }

    /// ストリーム終端を通知する。以降、バッファが空になったら EOF を返す
    fn close(&mut self) {
        self.closed = true;
    }

    /// 1バイト読み取り
    ///
    /// - バッファにデータがある → Ok(byte)
    /// - バッファ空 + closed → EndOfStream(0)
    /// - バッファ空 + !closed → Err(WouldBlock)
    fn read_byte(&mut self) -> Result<Option<u8>, ReadResult> {
        match self.data.pop_front() {
            Some(b) => Ok(Some(b)),
            None if self.closed => Ok(None), // EOF
            None => Err(ReadResult::WouldBlock),
        }
    }

    /// 改行を含む1行を読み取る
    ///
    /// - バッファに '\n' を含む行がある → Ok(line)
    /// - closed 後でバッファに残りデータ → Ok(残り全行)
    /// - closed 後でバッファ空 → Err(IoError("end of stream"))
    /// - バッファ不足 → Err(WouldBlock)
    fn read_line(&mut self) -> Result<String, ReadResult> {
        if let Some(newline_pos) = self.data.iter().position(|&b| b == b'\n') {
            let line: String = self.data.drain(..=newline_pos).map(|b| b as char).collect();
            return Ok(line);
        }
        if self.closed {
            if self.data.is_empty() {
                return Err(ReadResult::IoError("end of stream".to_string()));
            }
            // closed 後、残りのデータを最終行として返す
            let line: String = self.data.drain(..).map(|b| b as char).collect();
            return Ok(line);
        }
        Err(ReadResult::WouldBlock)
    }
}

/// 1命令の実行結果（内部使用）
#[derive(Debug)]
enum ExecuteResult {
    /// 次の命令へ進む
    Continue,
    /// プログラム終了 (Exit 命令)
    Exit,
    /// 実行時エラー
    Error(RuntimeError),
    /// 入力待ちで一時停止
    WaitingForInput(InputWaitType),
}

/// Whitespace 仮想マシン
///
/// 明示的スタックマシンとして全ての実行状態を保持する。
/// step() メソッドで指定ステップ数だけ実行し、自動的に中断する。
pub struct WhitespaceVM {
    // === プログラム ===
    /// 命令列
    instructions: Vec<Instruction>,
    /// ラベル → 命令インデックスのマッピング
    labels: HashMap<i64, usize>,

    // === 実行状態 ===
    /// プログラムカウンタ（次に実行する命令のインデックス）
    pc: usize,
    /// データスタック
    data_stack: Vec<i64>,
    /// コールスタック（サブルーチン call 時の戻りアドレス）
    call_stack: Vec<usize>,
    /// ヒープメモリ
    heap: HashMap<i64, i64>,

    // === I/O ===
    stdin: StdinSource,
    stdout: Box<dyn Write>,
    /// テスト用: stdout の内容を型安全に取得するための共有バッファ
    stdout_capture: Option<Rc<RefCell<Vec<u8>>>>,

    // === メトリクス ===
    /// 総実行命令数
    total_steps: usize,

    // === 拡張 API ===
    /// トレース記録（__trace 拡張 API の出力先）
    pub traced: BTreeMap<i64, i64>,
    /// デバッグ拡張 API が有効か (--std-ext debug)
    debug_ext: bool,

    // === 実行モード ===
    /// 未初期化ヒープアクセスをエラーにするか（wsc のデフォルト動作と同等）
    strict_heap: bool,
    /// 未初期化ヒープアクセス時にランダム値を返すか（初期値 0 依存のバグ検出用）
    /// strict_heap が true の場合はそちらが優先される
    randomize_heap: bool,

    // === 実行状態フラグ ===
    /// 実行完了済みかどうか
    completed: bool,

    // === プロファイリング ===
    /// プロファイリングモードが有効か
    profiling: bool,
    /// プロファイリング統計（profiling == true のときのみ更新される）
    profile_stats: ProfileStats,
}

impl WhitespaceVM {
    // === コンストラクタ ===

    /// Whitespace テキストから VM を構築
    pub fn from_source(source: &str) -> Result<Self, super::ParseError> {
        let instructions = super::parse(source)?;
        Self::from_instructions(instructions)
    }

    /// 命令列から VM を構築（重複ラベルチェック付き）
    pub fn from_instructions(instructions: Vec<Instruction>) -> Result<Self, super::ParseError> {
        let labels = Self::collect_labels(&instructions)?;

        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let writer = crate::base::shared_writer::SharedWriter(Rc::clone(&stdout_buf));

        Ok(Self {
            instructions,
            labels,
            pc: 0,
            data_stack: Vec::new(),
            call_stack: Vec::new(),
            heap: HashMap::new(),
            stdin: StdinSource::Buffered(Box::new(std::io::Cursor::new(Vec::new()))),
            stdout: Box::new(writer),
            stdout_capture: Some(stdout_buf),
            total_steps: 0,
            traced: BTreeMap::new(),
            debug_ext: false,
            strict_heap: false,
            randomize_heap: false,
            completed: false,
            profiling: false,
            profile_stats: ProfileStats::default(),
        })
    }

    /// I/O バッファを指定して構築
    pub fn with_io(mut self, stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self {
        self.stdin = StdinSource::Buffered(stdin);
        self.stdout = stdout;
        self.stdout_capture = None; // カスタム stdout では capture は無効化
        self
    }

    /// stdin のみを設定する（stdout の capture は維持）
    pub fn with_stdin(mut self, stdin: Box<dyn BufRead>) -> Self {
        self.stdin = StdinSource::Buffered(stdin);
        self
    }

    /// stdout のみを設定する（interactive stdin モードと併用）
    pub fn with_stdout(mut self, stdout: Box<dyn Write>) -> Self {
        self.stdout = stdout;
        self.stdout_capture = None; // カスタム stdout では capture は無効化
        self
    }

    /// Interactive stdin モードで構築（WASM 用）
    ///
    /// stdin バッファが空の場合に WaitingForInput を返し、VM を一時停止する。
    /// provide_stdin() で後からデータを追加可能。
    pub fn with_interactive_stdin(mut self) -> Self {
        self.stdin = StdinSource::Interactive(InteractiveBuffer::new());
        self
    }

    /// Interactive stdin にデータを追加
    ///
    /// WaitingForInput 状態の後に呼び出し、次の step() で入力をリトライする。
    /// interactive モード以外では無効。
    pub fn provide_stdin(&mut self, data: &str) {
        if let StdinSource::Interactive(buffer) = &mut self.stdin {
            buffer.append(data.as_bytes());
        }
    }

    /// Interactive stdin のストリーム終端を通知する
    ///
    /// 以降、バッファが空の状態で入力命令に到達すると EOF として扱われる。
    /// interactive モード以外では無効。
    pub fn close_stdin(&mut self) {
        if let StdinSource::Interactive(buffer) = &mut self.stdin {
            buffer.close();
        }
    }

    /// デバッグ拡張を有効にして構築
    pub fn with_debug_ext(mut self, enabled: bool) -> Self {
        self.debug_ext = enabled;
        self
    }

    /// strict-heap モードを有効にして構築
    ///
    /// 有効時、Store されていないアドレスへの Retrieve は UninitializedHeap エラーになる。
    /// wsc のデフォルト動作と同等の挙動を提供する。
    pub fn with_strict_heap(mut self, enabled: bool) -> Self {
        self.strict_heap = enabled;
        self
    }

    /// randomize-heap モードを有効にして構築
    ///
    /// 有効時、Store されていないアドレスへの Retrieve はランダム値を返す。
    /// strict_heap が有効の場合はそちらが優先される。
    pub fn with_randomize_heap(mut self, enabled: bool) -> Self {
        self.randomize_heap = enabled;
        self
    }

    /// プロファイリングモードを有効にして構築
    ///
    /// 有効時、命令カウント・ヒープアクセス統計・スタック深さを収集する。
    /// 収集した統計は `profile_stats()` で取得する。
    pub fn with_profiling(mut self, enabled: bool) -> Self {
        self.profiling = enabled;
        self
    }

    /// プロファイリング統計を取得する
    ///
    /// `with_profiling(true)` を指定した場合のみ有効なデータが返る。
    pub fn profile_stats(&self) -> &ProfileStats {
        &self.profile_stats
    }

    // === 実行 ===

    /// 指定ステップ数だけ実行し、結果を返す
    ///
    /// - budget > 0: 最大 budget 命令を実行
    /// - 途中で Exit/Error に到達した場合は即座に返す
    /// - budget を消費しきった場合は Suspended を返す
    pub fn step(&mut self, budget: usize) -> StepResult {
        if self.completed {
            return StepResult::Complete;
        }

        for _ in 0..budget {
            if self.pc >= self.instructions.len() {
                return StepResult::Error(RuntimeError::ProgramCounterOutOfBounds);
            }

            // プロファイリング有効時: 実行前に命令を記録
            if self.profiling {
                self.record_instruction_count(self.pc);
                // スタック深さの最大値を更新
                let ds = self.data_stack.len();
                let cs = self.call_stack.len();
                if ds > self.profile_stats.stack.max_data_stack_depth {
                    self.profile_stats.stack.max_data_stack_depth = ds;
                }
                if cs > self.profile_stats.stack.max_call_stack_depth {
                    self.profile_stats.stack.max_call_stack_depth = cs;
                }
            }

            match self.execute_instruction() {
                ExecuteResult::Continue => {
                    self.total_steps += 1;
                }
                ExecuteResult::Exit => {
                    self.completed = true;
                    return StepResult::Complete;
                }
                ExecuteResult::Error(e) => {
                    return StepResult::Error(e);
                }
                ExecuteResult::WaitingForInput(input_type) => {
                    return StepResult::WaitingForInput(input_type);
                }
            }
        }

        StepResult::Suspended
    }

    /// 命令ひとつのカウントを記録する（プロファイリング内部用）
    fn record_instruction_count(&mut self, pc: usize) {
        let counts = &mut self.profile_stats.instruction_counts;
        match &self.instructions[pc] {
            Instruction::Push(_) => counts.push += 1,
            Instruction::Duplicate => counts.duplicate += 1,
            Instruction::Copy(_) => counts.copy += 1,
            Instruction::Swap => counts.swap += 1,
            Instruction::Discard => counts.discard += 1,
            Instruction::Add => counts.add += 1,
            Instruction::Sub => counts.sub += 1,
            Instruction::Mul => counts.mul += 1,
            Instruction::Div => counts.div += 1,
            Instruction::Mod => counts.modulo += 1,
            Instruction::Store => counts.store += 1,
            Instruction::Retrieve => counts.retrieve += 1,
            Instruction::Label(_) => counts.label += 1,
            Instruction::Call(_) => counts.call += 1,
            Instruction::Jump(_) => counts.jump += 1,
            Instruction::JumpIfZero(_) => counts.jump_if_zero += 1,
            Instruction::JumpIfNegative(_) => counts.jump_if_negative += 1,
            Instruction::Return => counts.return_count += 1,
            Instruction::Exit => counts.exit += 1,
            Instruction::OutputChar => counts.output_char += 1,
            Instruction::OutputNumber => counts.output_number += 1,
            Instruction::InputChar => counts.input_char += 1,
            Instruction::InputNumber => counts.input_number += 1,
        }
    }

    /// 完了まで一括実行（最大ステップ制限付き）
    pub fn run(&mut self, max_steps: usize) -> StepResult {
        self.step(max_steps)
    }

    // === 状態参照 ===

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool {
        self.completed
    }

    /// データスタックの現在の状態
    pub fn data_stack(&self) -> &[i64] {
        &self.data_stack
    }

    /// ヒープの現在の状態
    pub fn heap(&self) -> &HashMap<i64, i64> {
        &self.heap
    }

    /// 総実行命令数
    pub fn total_steps(&self) -> usize {
        self.total_steps
    }

    /// 現在のプログラムカウンタ（次に実行する命令のインデックス）
    pub fn pc(&self) -> usize {
        self.pc
    }

    /// コールスタックの深さ
    pub fn call_stack_depth(&self) -> usize {
        self.call_stack.len()
    }

    /// 現在の命令のニーモニック表現を取得（デバッグ用）
    pub fn current_instruction(&self) -> Option<String> {
        if self.pc >= self.instructions.len() {
            None
        } else {
            Some(format!("{:?}", self.instructions[self.pc]))
        }
    }

    /// 命令列全体のニーモニック表現を取得
    pub fn disassemble(&self) -> Vec<String> {
        self.instructions
            .iter()
            .map(|inst| format!("{:?}", inst))
            .collect()
    }

    /// stdout の内容をフラッシュ
    pub fn flush(&mut self) {
        let _ = self.stdout.flush();
    }

    /// stdout の内容を文字列として取得（テスト用）
    ///
    /// デフォルトの stdout バッファ（SharedWriter）を使用している場合のみ動作する。
    /// `with_io` や `with_stdout` でカスタム stdout を設定した場合は空文字列を返す。
    pub fn get_stdout_string(&self) -> String {
        match &self.stdout_capture {
            Some(buf) => String::from_utf8_lossy(&buf.borrow()).to_string(),
            None => String::new(),
        }
    }

    /// ラベル収集（重複チェック付き）
    fn collect_labels(
        instructions: &[Instruction],
    ) -> Result<HashMap<i64, usize>, super::ParseError> {
        let mut labels = HashMap::new();
        for (i, inst) in instructions.iter().enumerate() {
            if let Instruction::Label(id) = inst {
                let label_value = id.to_ws_value();
                if let Some(&first_pos) = labels.get(&label_value) {
                    return Err(super::ParseError::DuplicateLabel {
                        label_id: label_value,
                        first_position: first_pos,
                        second_position: i,
                    });
                }
                labels.insert(label_value, i);
            }
        }
        Ok(labels)
    }

    /// 1命令を実行する
    fn execute_instruction(&mut self) -> ExecuteResult {
        let pc = self.pc;
        let inst = self.instructions[pc].clone(); // 借用問題を避けるため clone

        match inst {
            // === スタック操作 ===
            Instruction::Push(n) => {
                self.data_stack.push(n.0);
                self.pc += 1;
            }
            Instruction::Duplicate => {
                let val = match self.stack_top() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                self.data_stack.push(val);
                self.pc += 1;
            }
            Instruction::Copy(n) => {
                let idx = match self
                    .data_stack
                    .len()
                    .checked_sub(1 + n.0.unsigned_abs() as usize)
                {
                    Some(i) => i,
                    None => return ExecuteResult::Error(RuntimeError::StackUnderflow),
                };
                let val = self.data_stack[idx];
                self.data_stack.push(val);
                self.pc += 1;
            }
            Instruction::Swap => {
                let len = self.data_stack.len();
                if len < 2 {
                    return ExecuteResult::Error(RuntimeError::StackUnderflow);
                }
                self.data_stack.swap(len - 1, len - 2);
                self.pc += 1;
            }
            Instruction::Discard => {
                if let Err(e) = self.stack_pop() {
                    return ExecuteResult::Error(e);
                }
                self.pc += 1;
            }

            // === 算術演算 ===
            Instruction::Add => {
                if let Err(e) = self.binary_op(|a, b| Ok(a + b)) {
                    return ExecuteResult::Error(e);
                }
            }
            Instruction::Sub => {
                if let Err(e) = self.binary_op(|a, b| Ok(a - b)) {
                    return ExecuteResult::Error(e);
                }
            }
            Instruction::Mul => {
                if let Err(e) = self.binary_op(|a, b| Ok(a * b)) {
                    return ExecuteResult::Error(e);
                }
            }
            Instruction::Div => {
                if let Err(e) = self.binary_op(|a, b| {
                    if b == 0 {
                        Err(RuntimeError::DivisionByZero)
                    } else {
                        Ok(a / b)
                    }
                }) {
                    return ExecuteResult::Error(e);
                }
            }
            Instruction::Mod => {
                if let Err(e) = self.binary_op(|a, b| {
                    if b == 0 {
                        Err(RuntimeError::DivisionByZero)
                    } else {
                        Ok(a % b)
                    }
                }) {
                    return ExecuteResult::Error(e);
                }
            }

            // === ヒープアクセス ===
            Instruction::Store => {
                let val = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                let addr = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                if let Err(e) = self.heap_store(addr, val) {
                    return ExecuteResult::Error(e);
                }
                self.pc += 1;
            }
            Instruction::Retrieve => {
                let addr = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                let val = match self.heap_retrieve(addr) {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                self.data_stack.push(val);
                self.pc += 1;
            }

            // === フロー制御 ===
            Instruction::Label(_) => {
                // ラベルは実行時に何もしない（初期化時に収集済み）
                self.pc += 1;
            }
            Instruction::Call(id) => {
                self.call_stack.push(self.pc + 1);
                self.pc = match self.resolve_label(&id) {
                    Ok(pc) => pc,
                    Err(e) => return ExecuteResult::Error(e),
                };
            }
            Instruction::Jump(id) => {
                self.pc = match self.resolve_label(&id) {
                    Ok(pc) => pc,
                    Err(e) => return ExecuteResult::Error(e),
                };
            }
            Instruction::JumpIfZero(id) => {
                let val = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                if val == 0 {
                    self.pc = match self.resolve_label(&id) {
                        Ok(pc) => pc,
                        Err(e) => return ExecuteResult::Error(e),
                    };
                } else {
                    self.pc += 1;
                }
            }
            Instruction::JumpIfNegative(id) => {
                let val = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                if val < 0 {
                    self.pc = match self.resolve_label(&id) {
                        Ok(pc) => pc,
                        Err(e) => return ExecuteResult::Error(e),
                    };
                } else {
                    self.pc += 1;
                }
            }
            Instruction::Return => {
                self.pc = match self.call_stack.pop() {
                    Some(pc) => pc,
                    None => return ExecuteResult::Error(RuntimeError::CallStackUnderflow),
                };
            }
            Instruction::Exit => {
                return ExecuteResult::Exit;
            }

            // === I/O ===
            Instruction::OutputChar => {
                let val = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                if let Err(e) = write!(self.stdout, "{}", (val as u8) as char) {
                    return ExecuteResult::Error(RuntimeError::IoError(e.to_string()));
                }
                self.pc += 1;
            }
            Instruction::OutputNumber => {
                let val = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                if let Err(e) = write!(self.stdout, "{}", val) {
                    return ExecuteResult::Error(RuntimeError::IoError(e.to_string()));
                }
                self.pc += 1;
            }
            Instruction::InputChar => {
                let addr = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                match self.read_char() {
                    Ok(v) => {
                        self.heap.insert(addr, v);
                        self.pc += 1;
                    }
                    Err(ReadResult::WouldBlock) => {
                        // バッファ不足: スタックにアドレスを戻して一時停止
                        // 次の step() で同一命令をリトライする
                        self.data_stack.push(addr);
                        return ExecuteResult::WaitingForInput(InputWaitType::Char);
                    }
                    Err(ReadResult::IoError(e)) => {
                        return ExecuteResult::Error(RuntimeError::IoError(e));
                    }
                }
            }
            Instruction::InputNumber => {
                let addr = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                match self.read_number() {
                    Ok(v) => {
                        self.heap.insert(addr, v);
                        self.pc += 1;
                    }
                    Err(ReadResult::WouldBlock) => {
                        // バッファ不足: スタックにアドレスを戻して一時停止
                        self.data_stack.push(addr);
                        return ExecuteResult::WaitingForInput(InputWaitType::Number);
                    }
                    Err(ReadResult::IoError(e)) => {
                        return ExecuteResult::Error(RuntimeError::IoError(e));
                    }
                }
            }
        }

        ExecuteResult::Continue
    }

    /// スタックからポップ
    fn stack_pop(&mut self) -> Result<i64, RuntimeError> {
        self.data_stack.pop().ok_or(RuntimeError::StackUnderflow)
    }

    /// スタックのトップを参照（ポップしない）
    fn stack_top(&self) -> Result<i64, RuntimeError> {
        self.data_stack
            .last()
            .copied()
            .ok_or(RuntimeError::StackUnderflow)
    }

    /// 二項演算のヘルパー
    fn binary_op<F>(&mut self, op: F) -> Result<(), RuntimeError>
    where
        F: FnOnce(i64, i64) -> Result<i64, RuntimeError>,
    {
        let b = self.stack_pop()?;
        let a = self.stack_pop()?;
        let result = op(a, b)?;
        self.data_stack.push(result);
        self.pc += 1;
        Ok(())
    }

    /// ラベルを解決
    fn resolve_label(&self, id: &LabelId) -> Result<usize, RuntimeError> {
        let key = id.to_ws_value();
        self.labels
            .get(&key)
            .copied()
            .ok_or(RuntimeError::UndefinedLabel(key))
    }

    /// ヒープへの書き込み（拡張 API フック付き）
    fn heap_store(&mut self, addr: i64, val: i64) -> Result<(), RuntimeError> {
        if self.debug_ext {
            match addr {
                -10 => {
                    // __trace(val)
                    let traced = &mut self.traced;
                    if let Some(v) = traced.get_mut(&val) {
                        *v += 1;
                    } else {
                        traced.insert(val, 1);
                    }
                    return Ok(());
                }
                -11 => {
                    // __assert(val): val == 0 ならエラー
                    if val == 0 {
                        return Err(RuntimeError::AssertionFailed(val));
                    }
                    return Ok(());
                }
                -12 => {
                    // __assert_not(val): val != 0 ならエラー
                    if val != 0 {
                        return Err(RuntimeError::AssertionFailed(val));
                    }
                    return Ok(());
                }
                _ => {}
            }
        }
        // プロファイリング: ヒープ Store アドレスを記録
        if self.profiling {
            self.profile_stats.heap.record_store(addr);
        }
        // 通常のヒープ書き込み（debug_ext 無効時、または上記にマッチしないアドレス）
        self.heap.insert(addr, val);
        Ok(())
    }

    /// ヒープからの読み出し
    fn heap_retrieve(&mut self, addr: i64) -> Result<i64, RuntimeError> {
        // プロファイリング: ヒープ Retrieve アドレスを記録
        if self.profiling {
            self.profile_stats.heap.record_retrieve(addr);
        }
        match self.heap.get(&addr) {
            Some(&val) => Ok(val),
            None => {
                if self.strict_heap {
                    // strict-heap モード: 未初期化アドレスへのアクセスはエラー。strict_heap が最優先
                    Err(RuntimeError::UninitializedHeap(addr))
                } else if self.randomize_heap {
                    // randomize-heap モード: アドレスベースの決定論的な非自明値を返す
                    Ok(random_heap_fill(addr))
                } else {
                    // 通常モード: 未初期化アドレスは 0 を返す（Whitespace の一般的な挙動）
                    Ok(0)
                }
            }
        }
    }

    /// 標準入力から1文字を読み取り、その文字コードを返す
    fn read_char(&mut self) -> Result<i64, ReadResult> {
        match &mut self.stdin {
            StdinSource::Buffered(reader) => {
                let mut buf = [0u8; 1];
                match reader.read(&mut buf) {
                    Ok(1) => Ok(buf[0] as i64),
                    Ok(_) => Ok(0), // EOF
                    Err(e) => Err(ReadResult::IoError(e.to_string())),
                }
            }
            StdinSource::Interactive(buffer) => match buffer.read_byte() {
                Ok(Some(b)) => Ok(b as i64),
                Ok(None) => Ok(0), // EOF
                Err(e) => Err(e),
            },
        }
    }

    /// 標準入力から整数を読み取る
    fn read_number(&mut self) -> Result<i64, ReadResult> {
        match &mut self.stdin {
            StdinSource::Buffered(reader) => {
                let mut line = String::new();
                reader
                    .read_line(&mut line)
                    .map_err(|e| ReadResult::IoError(e.to_string()))?;
                line.trim()
                    .parse::<i64>()
                    .map_err(|e| ReadResult::IoError(e.to_string()))
            }
            StdinSource::Interactive(buffer) => match buffer.read_line() {
                Ok(line) => line
                    .trim()
                    .parse::<i64>()
                    .map_err(|e| ReadResult::IoError(e.to_string())),
                Err(e) => Err(e),
            },
        }
    }
}

/// 未初期化ヒープのフィル値（アドレスベースの決定論的な値）
///
/// 同じアドレスには常に同じ値を返す。
/// 初期値 0 への暗黙依存バグを検出しやすくするため、0 ではない非自明値を生成する。
fn random_heap_fill(addr: i64) -> i64 {
    (addr as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_ws::types::WsNumber;

    #[test]
    fn test_push_and_add() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(2)),
            Instruction::Push(WsNumber(3)),
            Instruction::Add,
            Instruction::Exit,
        ])
        .unwrap();
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[5]);
    }

    #[test]
    fn test_suspension() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(1)),
            Instruction::Push(WsNumber(2)),
            Instruction::Push(WsNumber(3)),
            Instruction::Add,
            Instruction::Add,
            Instruction::Exit,
        ])
        .unwrap();
        // budget=2 で中断
        let result = vm.step(2);
        assert_eq!(result, StepResult::Suspended);
        assert_eq!(vm.data_stack(), &[1, 2]); // 2命令分のみ実行

        // 残りを実行
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[6]);
    }

    #[test]
    fn test_subroutine_call() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            // 0: jump to label 1
            Instruction::Jump(LabelId(1)),
            // 1: subroutine at label 2
            Instruction::Label(LabelId(2)),
            Instruction::Push(WsNumber(42)),
            Instruction::Return,
            // 4: main code at label 1
            Instruction::Label(LabelId(1)),
            Instruction::Call(LabelId(2)),
            Instruction::Exit,
        ])
        .unwrap();
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[42]);
    }

    #[test]
    fn test_trace_extension() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            // __trace(7): push -10, push 7, store
            Instruction::Push(WsNumber(-10)),
            Instruction::Push(WsNumber(7)),
            Instruction::Store,
            Instruction::Exit,
        ])
        .unwrap()
        .with_debug_ext(true);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.traced.get(&7), Some(&1));
    }

    #[test]
    fn test_heap_store_retrieve() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(100)), // addr
            Instruction::Push(WsNumber(42)),  // value
            Instruction::Store,
            Instruction::Push(WsNumber(100)), // addr
            Instruction::Retrieve,
            Instruction::Exit,
        ])
        .unwrap();
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[42]);
    }

    #[test]
    fn test_division_by_zero() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(10)),
            Instruction::Push(WsNumber(0)),
            Instruction::Div,
            Instruction::Exit,
        ])
        .unwrap();
        let result = vm.run(100);
        assert_eq!(result, StepResult::Error(RuntimeError::DivisionByZero));
    }

    #[test]
    fn test_stack_underflow() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Add, // スタックが空なのに Add
            Instruction::Exit,
        ])
        .unwrap();
        let result = vm.run(100);
        assert_eq!(result, StepResult::Error(RuntimeError::StackUnderflow));
    }

    #[test]
    fn test_strict_heap_uninitialized_error() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(100)), // addr
            Instruction::Retrieve,            // 未初期化
            Instruction::Exit,
        ])
        .unwrap()
        .with_strict_heap(true);
        let result = vm.run(100);
        assert_eq!(
            result,
            StepResult::Error(RuntimeError::UninitializedHeap(100))
        );
    }

    #[test]
    fn test_strict_heap_initialized_ok() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(100)), // addr
            Instruction::Push(WsNumber(42)),  // value
            Instruction::Store,
            Instruction::Push(WsNumber(100)), // addr
            Instruction::Retrieve,
            Instruction::Exit,
        ])
        .unwrap()
        .with_strict_heap(true);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[42]);
    }

    #[test]
    fn test_non_strict_heap_uninitialized_returns_zero() {
        // 既存動作の確認: strict-heap 無効時は未初期化アドレスに 0 を返す
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(100)),
            Instruction::Retrieve,
            Instruction::Exit,
        ])
        .unwrap();
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[0]);
    }

    // ===== interactive stdin テスト =====

    /// InputChar: interactive モードでバッファにデータがある場合は正常読み取り
    #[test]
    fn test_interactive_stdin_input_char_ok() {
        // InputChar: スタックトップのアドレスへ文字コードを格納する
        // addr=10 に 'A'(65) が格納されることを確認
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(10)), // addr
            Instruction::InputChar,
            Instruction::Push(WsNumber(10)), // addr
            Instruction::Retrieve,
            Instruction::Exit,
        ])
        .unwrap()
        .with_interactive_stdin();

        vm.provide_stdin("A");
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[65]); // 'A' = 65
    }

    /// InputChar: バッファが空の場合は WaitingForInput(Char) を返す
    #[test]
    fn test_interactive_stdin_input_char_waiting() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(10)),
            Instruction::InputChar,
            Instruction::Exit,
        ])
        .unwrap()
        .with_interactive_stdin();

        // バッファ空で実行 → WaitingForInput
        let result = vm.step(100);
        assert_eq!(result, StepResult::WaitingForInput(InputWaitType::Char));

        // データ追加後に resume → Complete
        vm.provide_stdin("B");
        let result = vm.step(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(*vm.heap().get(&10).unwrap(), 66); // 'B' = 66
    }

    /// InputNumber: interactive モードでバッファに改行付き数値がある場合は正常読み取り
    #[test]
    fn test_interactive_stdin_input_number_ok() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(20)), // addr
            Instruction::InputNumber,
            Instruction::Push(WsNumber(20)),
            Instruction::Retrieve,
            Instruction::Exit,
        ])
        .unwrap()
        .with_interactive_stdin();

        vm.provide_stdin("42\n");
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[42]);
    }

    /// InputNumber: バッファに改行なしは WaitingForInput(Number) を返す
    #[test]
    fn test_interactive_stdin_input_number_waiting() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(20)),
            Instruction::InputNumber,
            Instruction::Exit,
        ])
        .unwrap()
        .with_interactive_stdin();

        // バッファ空で実行 → WaitingForInput
        let result = vm.step(100);
        assert_eq!(result, StepResult::WaitingForInput(InputWaitType::Number));

        // 改行なしでデータ追加しても WaitingForInput のまま
        vm.provide_stdin("10");
        let result = vm.step(100);
        assert_eq!(result, StepResult::WaitingForInput(InputWaitType::Number));

        // 改行追加で resume
        vm.provide_stdin("\n");
        let result = vm.step(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(*vm.heap().get(&20).unwrap(), 10);
    }

    /// InputChar: close_stdin 後にバッファ空 → EOF (0)
    #[test]
    fn test_interactive_stdin_input_char_eof_after_close() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(10)),
            Instruction::InputChar,
            Instruction::Push(WsNumber(10)),
            Instruction::Retrieve,
            Instruction::Exit,
        ])
        .unwrap()
        .with_interactive_stdin();

        vm.close_stdin();
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(*vm.heap().get(&10).unwrap(), 0); // EOF = 0
    }

    /// 非 interactive モードは動作変更なし（既存動作の確認）
    #[test]
    fn test_non_interactive_stdin_unaffected() {
        let stdin_data = b"X".to_vec();
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(5)),
            Instruction::InputChar,
            Instruction::Push(WsNumber(5)),
            Instruction::Retrieve,
            Instruction::Exit,
        ])
        .unwrap()
        .with_io(
            Box::new(std::io::BufReader::new(std::io::Cursor::new(stdin_data))),
            Box::new(Vec::<u8>::new()),
        );

        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(*vm.heap().get(&5).unwrap(), 88); // 'X' = 88
    }
}
