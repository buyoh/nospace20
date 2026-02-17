//! Whitespace インタプリタ
//!
//! Whitespace 命令列を実行するスタックマシン。
//! 全ての実行状態を明示的に保持し、中断・再開可能。

use crate::compiler_ws::instruction::Instruction;
use crate::compiler_ws::types::LabelId;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};

/// VM の実行結果
#[derive(Debug, PartialEq, Eq)]
pub enum StepResult {
    /// 実行継続中（バジェット消費で中断）
    Suspended,
    /// 正常終了（Exit 命令到達）
    Complete,
    /// 実行時エラー
    Error(RuntimeError),
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

/// 1命令の実行結果（内部使用）
#[derive(Debug)]
enum ExecuteResult {
    /// 次の命令へ進む
    Continue,
    /// プログラム終了 (Exit 命令)
    Exit,
    /// 実行時エラー
    Error(RuntimeError),
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
    stdin: Box<dyn BufRead>,
    stdout: Box<dyn Write>,

    // === メトリクス ===
    /// 総実行命令数
    total_steps: usize,

    // === 拡張 API ===
    /// トレース記録（__trace 拡張 API の出力先）
    pub traced: BTreeMap<i64, i64>,
    /// デバッグ拡張 API が有効か (--std-ext debug)
    debug_ext: bool,

    // === 実行状態フラグ ===
    /// 実行完了済みかどうか
    completed: bool,
}

impl WhitespaceVM {
    // === コンストラクタ ===

    /// Whitespace テキストから VM を構築
    pub fn from_source(source: &str) -> Result<Self, super::ParseError> {
        let instructions = super::parse(source)?;
        Ok(Self::from_instructions(instructions))
    }

    /// 命令列から VM を構築（compiler_ws のパイプライン用）
    pub fn from_instructions(instructions: Vec<Instruction>) -> Self {
        let labels = Self::collect_labels(&instructions);

        Self {
            instructions,
            labels,
            pc: 0,
            data_stack: Vec::new(),
            call_stack: Vec::new(),
            heap: HashMap::new(),
            stdin: Box::new(std::io::Cursor::new(Vec::new())),
            stdout: Box::new(Vec::<u8>::new()),
            total_steps: 0,
            traced: BTreeMap::new(),
            debug_ext: false,
            completed: false,
        }
    }

    /// I/O バッファを指定して構築
    pub fn with_io(mut self, stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self {
        self.stdin = stdin;
        self.stdout = stdout;
        self
    }

    /// デバッグ拡張を有効にして構築
    pub fn with_debug_ext(mut self, enabled: bool) -> Self {
        self.debug_ext = enabled;
        self
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
            }
        }

        StepResult::Suspended
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
    pub fn get_stdout_string(&self) -> String {
        // stdout が Vec<u8> の場合のみ動作
        let stdout_ref = &self.stdout;
        let bytes: &Vec<u8> =
            unsafe { &*(stdout_ref as *const Box<dyn Write> as *const Box<Vec<u8>>) };
        String::from_utf8_lossy(bytes).to_string()
    }

    // === 内部処理 ===

    /// ラベル収集
    fn collect_labels(instructions: &[Instruction]) -> HashMap<i64, usize> {
        let mut labels = HashMap::new();
        for (i, inst) in instructions.iter().enumerate() {
            if let Instruction::Label(id) = inst {
                labels.insert(id.to_ws_value(), i);
            }
        }
        labels
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
                let val = match self.read_char() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                self.heap.insert(addr, val);
                self.pc += 1;
            }
            Instruction::InputNumber => {
                let addr = match self.stack_pop() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                let val = match self.read_number() {
                    Ok(v) => v,
                    Err(e) => return ExecuteResult::Error(e),
                };
                self.heap.insert(addr, val);
                self.pc += 1;
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
        // 通常のヒープ書き込み（debug_ext 無効時、または上記にマッチしないアドレス）
        self.heap.insert(addr, val);
        Ok(())
    }

    /// ヒープからの読み出し
    fn heap_retrieve(&self, addr: i64) -> Result<i64, RuntimeError> {
        // 未初期化アドレスは 0 を返す（Whitespace の一般的な挙動）
        Ok(*self.heap.get(&addr).unwrap_or(&0))
    }

    /// 標準入力から1文字を読み取り、その文字コードを返す
    fn read_char(&mut self) -> Result<i64, RuntimeError> {
        let mut buf = [0u8; 1];
        match self.stdin.read(&mut buf) {
            Ok(1) => Ok(buf[0] as i64),
            Ok(_) => Ok(0), // EOF
            Err(e) => Err(RuntimeError::IoError(e.to_string())),
        }
    }

    /// 標準入力から整数を読み取る
    fn read_number(&mut self) -> Result<i64, RuntimeError> {
        let mut line = String::new();
        self.stdin
            .read_line(&mut line)
            .map_err(|e| RuntimeError::IoError(e.to_string()))?;
        line.trim()
            .parse::<i64>()
            .map_err(|e| RuntimeError::IoError(e.to_string()))
    }
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
        ]);
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
        ]);
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
        ]);
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
        ]);
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
        ]);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Error(RuntimeError::DivisionByZero));
    }

    #[test]
    fn test_stack_underflow() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Add, // スタックが空なのに Add
            Instruction::Exit,
        ]);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Error(RuntimeError::StackUnderflow));
    }
}
