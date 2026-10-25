//! Whitespace インタプリタ
//!
//! Whitespace 命令列を実行するスタックマシン。
//! 全ての実行状態を明示的に保持し、中断・再開可能。

use crate::base::ws_types::{Instruction, LabelId};
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
    pub slide: usize,
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
    /// ヒープ書き込み（Store 命令、InputChar / InputNumber の格納）の回数
    pub store_count: usize,
    /// Retrieve 命令の実行回数
    pub retrieve_count: usize,
    /// ヒープ書き込み（Store / 入力命令）したアドレスの (最小値, 最大値)。書き込みが0回なら None
    pub store_range: Option<(i64, i64)>,
    /// Retrieve したアドレスの (最小値, 最大値)。Retrieve が0回なら None
    pub retrieve_range: Option<(i64, i64)>,
    /// Store または Retrieve でアクセスしたユニークアドレス数
    pub unique_address_count: usize,
    /// ユニークアドレスを追跡する内部セット
    unique_addresses: BTreeSet<i64>,
}

impl HeapProfileStats {
    /// ヒープ書き込み（Store / 入力命令）のアドレスを記録する
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
///
/// NOTE: 型定義は `base::error::ws_error::WsRuntimeError` に移動。
/// 後方互換のため type alias として公開。
pub use crate::base::error::ws_error::WsRuntimeError;
pub type RuntimeError = WsRuntimeError;

/// read_char / read_number の失敗種別（内部使用）
#[derive(Debug)]
enum ReadResult {
    /// stdin バッファ不足（interactive モード用）
    WouldBlock,
    /// 実行時エラー（I/O の失敗・入力の終端・不正な数値の入力）
    Error(RuntimeError),
}

/// 数値の入力で、何も読めずに入力が終わったときのエラー文言（Buffered / Interactive で共通）
const UNEXPECTED_END_OF_INPUT: &str = "unexpected end of input";

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

    /// 改行を含む1行をバイト列として読み取る
    ///
    /// - バッファに '\n' を含む行がある → Ok(line)
    /// - closed 後でバッファに残りデータ → Ok(残り全行)
    /// - closed 後でバッファ空 → Ok(None)（EOF）
    /// - バッファ不足 → Err(WouldBlock)
    // BUG FIX: 従来は各バイトを `b as char`（Latin-1）で文字列にしていたため、UTF-8 の空白
    // （NBSP など）が Buffered 経路（UTF-8 で読む）と違う文字になり、`__geti` の結果が食い違った。
    // バイト列のまま返し、UTF-8 への変換と解釈は read_number で両経路共通に行う。
    fn read_line(&mut self) -> Result<Option<Vec<u8>>, ReadResult> {
        if let Some(newline_pos) = self.data.iter().position(|&b| b == b'\n') {
            return Ok(Some(self.data.drain(..=newline_pos).collect()));
        }
        if self.closed {
            if self.data.is_empty() {
                return Ok(None);
            }
            // closed 後、残りのデータを最終行として返す
            return Ok(Some(self.data.drain(..).collect()));
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
    /// __clog 拡張 API (-13) の出力先。None なら出力を捨てる
    clog_writer: Option<Box<dyn Write>>,

    // === 資源の上限 ===
    /// VM が使うメモリ（ヒープ・データスタック・戻り先の合計セル数）の上限。None は無制限
    max_memory_cells: Option<usize>,

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
            clog_writer: None,
            max_memory_cells: None,
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

    /// __clog 拡張 API (-13) の出力先を設定して構築
    ///
    /// プログラムの stdout とは別の writer。未設定なら __clog の出力は捨てられる。
    /// 書き込みエラーは無視する（デバッグ用の補助出力で実行を止めないため）。
    pub fn with_clog_writer(mut self, writer: Box<dyn Write>) -> Self {
        self.clog_writer = Some(writer);
        self
    }

    /// VM が使うメモリ（ヒープ・データスタック・戻り先の合計セル数）の上限を設定する。None は無制限
    ///
    /// 上限を超える操作（ヒープの新しいアドレスへの書き込み、データスタックへの push、
    /// Call での戻り先の積み込み）は行わず、`MemoryLimitExceeded` エラーになる。
    /// `Some(0)` は「上限 0 セル」であり、無制限ではない。
    pub fn with_max_memory_cells(mut self, limit: Option<usize>) -> Self {
        self.max_memory_cells = limit;
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
            Instruction::Slide(_) => counts.slide += 1,
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

    /// stdout をフラッシュし、失敗を返す
    ///
    /// `flush` は失敗を捨てるため、出力先への書き込みの失敗（`> /dev/full` など）を
    /// 終了コードに反映したい CLI はこちらを使う
    pub fn try_flush(&mut self) -> std::io::Result<()> {
        self.stdout.flush()
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
                if let Err(e) = self.stack_push_growing(n.0) {
                    return ExecuteResult::Error(e);
                }
                self.pc += 1;
            }
            Instruction::Duplicate => {
                let val = match self.stack_top() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                if let Err(e) = self.stack_push_growing(val) {
                    return ExecuteResult::Error(e);
                }
                self.pc += 1;
            }
            Instruction::Copy(n) => {
                // バグ修正: 従来は `n.0.unsigned_abs() as usize` で変換しており、wasm32 では上位ビットが
                // 切り捨てられて Copy(2^32) が Copy(0) として成功し、`1 + ...` の加算が溢れて trap していた。
                // 解決策: 深さは u64 のまま index_from_top で計算し、確定したインデックスだけを usize にする。
                // 負の n は |n| として扱う（本処理系の挙動。docs/spec-whitespace.md 参照）
                let idx = match index_from_top(self.data_stack.len(), n.0.unsigned_abs()) {
                    Some(i) => i,
                    None => return ExecuteResult::Error(RuntimeError::StackUnderflow),
                };
                let val = self.data_stack[idx];
                if let Err(e) = self.stack_push_growing(val) {
                    return ExecuteResult::Error(e);
                }
                self.pc += 1;
            }
            Instruction::Slide(n) => {
                // 先頭を残して、その下の n 個を取り除く。負の n は予約のためエラー（docs/spec-whitespace.md 参照）。
                // Copy と異なり |n| にはしない。深さは u64 のまま index_from_top に渡す
                let Ok(depth) = u64::try_from(n.0) else {
                    return ExecuteResult::Error(RuntimeError::StackUnderflow);
                };
                // 先頭から depth 番目のインデックス = 除去後に残る下側の要素数
                let keep_len = match index_from_top(self.data_stack.len(), depth) {
                    Some(i) => i,
                    None => return ExecuteResult::Error(RuntimeError::StackUnderflow),
                };
                // index_from_top が Some なら len >= depth + 1 >= 1 なので先頭は存在する
                let top = self.data_stack[self.data_stack.len() - 1];
                // 要素数は増えないため、メモリ上限の確認は不要
                self.data_stack.truncate(keep_len);
                self.data_stack.push(top);
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
            // バグ修正: 従来は素の演算子を使っていたため、オーバーフロー時に debug ビルドでは panic、
            // release ビルドでは wrap とビルド設定で挙動が変わり、i64::MIN / -1 と i64::MIN % -1 は
            // 常に panic（panic = "abort" のため CLI は abort、WASM は trap）していた。
            // 解決策: 本処理系の挙動（docs/spec.md「Appendix」）として定めたとおり、
            // base::pure_eval と同じ 64 ビット 2 の補数の wrapping 演算に統一する。
            // （言語仕様上、オーバーフロー時の挙動は実装依存）
            Instruction::Add => {
                if let Err(e) = self.binary_op(|a, b| Ok(a.wrapping_add(b))) {
                    return ExecuteResult::Error(e);
                }
            }
            Instruction::Sub => {
                if let Err(e) = self.binary_op(|a, b| Ok(a.wrapping_sub(b))) {
                    return ExecuteResult::Error(e);
                }
            }
            Instruction::Mul => {
                if let Err(e) = self.binary_op(|a, b| Ok(a.wrapping_mul(b))) {
                    return ExecuteResult::Error(e);
                }
            }
            Instruction::Div => {
                if let Err(e) = self.binary_op(|a, b| {
                    if b == 0 {
                        Err(RuntimeError::DivisionByZero)
                    } else {
                        Ok(a.wrapping_div(b))
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
                        Ok(a.wrapping_rem(b))
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
                // 未定義ラベルのエラーを優先するため、ラベルを解決してからメモリ上限を確認する
                let target_pc = match self.resolve_label(&id) {
                    Ok(pc) => pc,
                    Err(e) => return ExecuteResult::Error(e),
                };
                if let Err(e) = self.check_memory_growth() {
                    return ExecuteResult::Error(e);
                }
                self.call_stack.push(self.pc + 1);
                self.pc = target_pc;
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
                // バグ修正: `(val as u8) as char` は 128..=255 を UTF-8 2 バイトにしていた
                let bytes = match crate::base::putc::encode_putc(val) {
                    Some(b) => b,
                    None => {
                        return ExecuteResult::Error(RuntimeError::IoError(format!(
                            "outchar: invalid character code {}",
                            val
                        )))
                    }
                };
                if let Err(e) = self.stdout.write_all(&bytes) {
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
                        // バグ修正: 従来は self.heap.insert で直接書き込んでいたため、
                        // --std-ext debug の負アドレスフックとプロファイラ統計を迂回していた。
                        // 解決策: Store 命令と同じ heap_store を経由させる（メモリ上限の確認もここで行われる）。
                        if let Err(e) = self.heap_store(addr, v) {
                            return ExecuteResult::Error(e);
                        }
                        self.pc += 1;
                    }
                    Err(ReadResult::WouldBlock) => {
                        // バッファ不足: スタックにアドレスを戻して一時停止
                        // 次の step() で同一命令をリトライする
                        self.data_stack.push(addr);
                        return ExecuteResult::WaitingForInput(InputWaitType::Char);
                    }
                    Err(ReadResult::Error(e)) => {
                        return ExecuteResult::Error(e);
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
                        // バグ修正: InputChar と同じ（heap_store を経由させる）
                        if let Err(e) = self.heap_store(addr, v) {
                            return ExecuteResult::Error(e);
                        }
                        self.pc += 1;
                    }
                    Err(ReadResult::WouldBlock) => {
                        // バッファ不足: スタックにアドレスを戻して一時停止
                        self.data_stack.push(addr);
                        return ExecuteResult::WaitingForInput(InputWaitType::Number);
                    }
                    Err(ReadResult::Error(e)) => {
                        return ExecuteResult::Error(e);
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

    /// データスタックに正味で 1 要素を積む（Push / Duplicate / Copy 用）
    ///
    /// pop した後に push する命令（Retrieve・算術演算など）は使用セル数が増えないため、
    /// 上限の確認をせずに `data_stack.push` を直接使う。
    fn stack_push_growing(&mut self, val: i64) -> Result<(), RuntimeError> {
        self.check_memory_growth()?;
        self.data_stack.push(val);
        Ok(())
    }

    /// 使用セル数を 1 増やしても上限を超えないか確認する
    ///
    /// 上限が None のときは分岐 1 つだけで返る。
    fn check_memory_growth(&self) -> Result<(), RuntimeError> {
        match self.max_memory_cells {
            Some(limit) if self.exceeds_memory_limit_by_one(limit) => {
                Err(RuntimeError::MemoryLimitExceeded(limit))
            }
            _ => Ok(()),
        }
    }

    /// 現在の使用セル数 + 1 が上限を超えるか
    fn exceeds_memory_limit_by_one(&self, limit: usize) -> bool {
        let used_cells = self
            .heap
            .len()
            .checked_add(self.data_stack.len())
            .and_then(|n| n.checked_add(self.call_stack.len()))
            .and_then(|n| n.checked_add(1));
        match used_cells {
            Some(n) => n > limit,
            None => true,
        }
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

    /// ヒープへの書き込み（拡張 API フック・メモリ上限の確認付き）
    ///
    /// ヒープへの書き込みはすべてここを通す（Store・InputChar・InputNumber）。
    /// 新しく書き込む経路を追加するときも、`heap.insert` を直接呼ばずにここを使うこと。
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
                -13 => {
                    // __clog(val): clog writer に出力する（未設定なら捨てる）
                    return self.write_clog(val);
                }
                _ => {}
            }
        }
        // メモリ上限: 新しいアドレスへの書き込みだけが使用セル数を増やす。
        // debug 拡張のフックの後に置く（フックはヒープに入らないため、上限ちょうどでも動作させる）。
        // NOTE: 現行の命令（Store・入力命令）はオペランドを pop してから書き込むため、使用量は直前の
        //       push で確認済みの値を超えず、ここでは失敗しない。ヒープに書き込む経路が増えたときのための確認
        if let Some(limit) = self.max_memory_cells {
            if self.exceeds_memory_limit_by_one(limit) && !self.heap.contains_key(&addr) {
                return Err(RuntimeError::MemoryLimitExceeded(limit));
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

    /// __clog の出力
    ///
    /// 出力順を保つため、先にプログラムの stdout を flush する（flush の失敗は stdout の I/O エラー）。
    /// clog writer への書き込みエラーは無視する。
    fn write_clog(&mut self, val: i64) -> Result<(), RuntimeError> {
        if let Some(writer) = self.clog_writer.as_mut() {
            self.stdout
                .flush()
                .map_err(|e| RuntimeError::IoError(e.to_string()))?;
            let _ = writeln!(writer, "__clog: {}", val);
            let _ = writer.flush();
        }
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
                    Ok(crate::algorithm::hash::lcg_hash(addr as u64) as i64)
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
                    Err(e) => Err(ReadResult::Error(RuntimeError::IoError(e.to_string()))),
                }
            }
            StdinSource::Interactive(buffer) => match buffer.read_byte() {
                Ok(Some(b)) => Ok(b as i64),
                Ok(None) => Ok(0), // EOF
                Err(e) => Err(e),
            },
        }
    }

    /// 標準入力から 1 行を読み、整数として解釈する（wsc の readnum と同じ規則）
    ///
    /// 行の取得だけを経路（Buffered / Interactive）ごとに行い、解釈は `parse_input_number` で共通に行う。
    // BUG FIX: 従来は Buffered が `read_line`（不正な UTF-8 で I/O エラー）、Interactive が Latin-1 で
    // 文字列にしてから `str::parse::<i64>` していたため、経路で結果が食い違い、`1_000` など wsc が
    // 受け付ける入力も拒否していた。行はバイト列で受け取って UTF-8（不正なバイトは U+FFFD）に変換し、
    // 共通の解釈関数に渡す。
    fn read_number(&mut self) -> Result<i64, ReadResult> {
        let line = match &mut self.stdin {
            StdinSource::Buffered(reader) => {
                let mut line = Vec::new();
                let read_bytes = reader
                    .read_until(b'\n', &mut line)
                    .map_err(|e| ReadResult::Error(RuntimeError::IoError(e.to_string())))?;
                (read_bytes > 0).then_some(line)
            }
            StdinSource::Interactive(buffer) => buffer.read_line()?,
        };
        let line = line.ok_or_else(|| {
            ReadResult::Error(RuntimeError::IoError(UNEXPECTED_END_OF_INPUT.to_string()))
        })?;
        let line = String::from_utf8_lossy(&line);
        super::input_number::parse_input_number(&line).ok_or_else(|| {
            let shown = line.strip_suffix('\n').unwrap_or(&line);
            ReadResult::Error(RuntimeError::InvalidNumberInput(shown.to_string()))
        })
    }
}

/// スタック先頭から depth 番目（0 = 先頭）の要素のインデックスを返す。範囲外なら None
///
/// 深さの計算は VM の整数と同じ 64 bit（u64）で行い、usize への変換は
/// アクセスするインデックスが確定した後の 1 回だけにする（usize が 32 bit の wasm32 でも同じ判定になる）。
fn index_from_top(stack_len: usize, depth: u64) -> Option<usize> {
    // usize が 64 bit 以下のプラットフォームでは失敗しない（失敗は処理系の前提の破綻なので panic）
    let stack_len_u64 =
        u64::try_from(stack_len).expect("usize wider than 64 bits is not supported");
    let index = stack_len_u64.checked_sub(depth.checked_add(1)?)?;
    // index < stack_len（元は usize）のため、変換は失敗しない
    Some(usize::try_from(index).expect("index is less than stack_len"))
}

#[cfg(test)]
#[path = "interpreter_tests.rs"]
mod tests;
