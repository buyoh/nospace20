//! Whitespace VM の WASM ラッパー
//!
//! nospace ソースもしくは Whitespace ソースからステップ実行可能な VM を構築する。

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::whitespace::{InputWaitType, StepResult, WhitespaceVM};
use crate::{compile_to_ws, WsCompileOptions};

use super::pipeline;
use super::types::{
    JsNumberArray, JsNumberRecord, JsStdExtensionArray, JsStringArray, JsVmStepResult, ResultErr,
    VmStepResult,
};

// ========================================
// SharedWriter
// ========================================

/// `Rc<RefCell<Vec<u8>>>` をラップして `Write` トレイトを実装する
///
/// stdout バッファを VM と呼び出し元で共有するために使用する。
pub(super) struct SharedWriter(pub Rc<RefCell<Vec<u8>>>);

impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ========================================
// Whitespace VM ヘルパー
// ========================================

/// Whitespace ソースから VM を構築する共通ヘルパー
///
/// `from_whitespace` と `from_whitespace_interactive` の重複を解消する。
/// `interactive=true` のとき `with_interactive_stdin()` を適用する。
fn create_from_ws_source(
    ws_source: &str,
    initial_stdin: &str,
    interactive: bool,
) -> Result<WasmWhitespaceVM, JsValue> {
    let vm = WhitespaceVM::from_source(ws_source).map_err(|e| {
        let err = ResultErr::single_error(format!("Whitespace parse error: {:?}", e));
        serde_wasm_bindgen::to_value(&err).unwrap()
    })?;

    let vm = vm.with_debug_ext(false);
    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let stdout_clone = Rc::clone(&stdout_buf);

    if interactive {
        let vm = vm.with_interactive_stdin();
        let mut vm_with_io = vm.with_stdout(Box::new(SharedWriter(stdout_clone)));
        if !initial_stdin.is_empty() {
            vm_with_io.provide_stdin(initial_stdin);
        }
        Ok(WasmWhitespaceVM {
            vm: vm_with_io,
            stdout_buffer: stdout_buf,
        })
    } else {
        let stdin_buf = Box::new(std::io::BufReader::new(Cursor::new(
            initial_stdin.as_bytes().to_vec(),
        )));
        let vm_with_io = vm.with_io(stdin_buf, Box::new(SharedWriter(stdout_clone)));
        Ok(WasmWhitespaceVM {
            vm: vm_with_io,
            stdout_buffer: stdout_buf,
        })
    }
}

// ========================================
// WasmWhitespaceVM
// ========================================

/// Whitespace VM の WASM ラッパー
///
/// JS 側ではオペーク型として扱われ、メソッド呼び出しで状態を操作する。
#[wasm_bindgen]
pub struct WasmWhitespaceVM {
    vm: WhitespaceVM,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}

#[wasm_bindgen]
impl WasmWhitespaceVM {
    /// nospace ソースをコンパイルし、Whitespace VM を構築する
    ///
    /// - `std_extensions`: 有効にする拡張の配列（例: `["debug", "alloc"]`）
    #[wasm_bindgen(constructor)]
    pub fn new(
        nospace_source: &str,
        stdin: &str,
        interactive: Option<bool>,
        std_extensions: Option<JsStdExtensionArray>,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        let (debug_ext, alloc_ext) = pipeline::parse_std_extensions(std_extensions)
            .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        let (scope, text_code) = pipeline::analyze_source(nospace_source)
            .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        // コンパイル
        let ws_options = WsCompileOptions {
            debug_ext,
            alloc_ext,
            ..Default::default()
        };
        let ws_source = compile_to_ws(&scope, &ws_options)
            .map_err(|e| {
                serde_wasm_bindgen::to_value(&pipeline::convert_errors(&e, &text_code)).unwrap()
            })?;

        create_from_ws_source(&ws_source, stdin, interactive.unwrap_or(false))
    }

    /// Whitespace ソースコードから直接 VM を構築する
    #[wasm_bindgen(js_name = "fromWhitespace")]
    pub fn from_whitespace(ws_source: &str, stdin: &str) -> Result<WasmWhitespaceVM, JsValue> {
        create_from_ws_source(ws_source, stdin, false)
    }

    /// Interactive モードで Whitespace ソースから VM を構築する
    ///
    /// stdin バッファが空の場合、WaitingForInput で一時停止する。
    /// provide_stdin() で後からデータを追加可能。
    #[wasm_bindgen(js_name = "fromWhitespaceInteractive")]
    pub fn from_whitespace_interactive(
        ws_source: &str,
        initial_stdin: &str,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        create_from_ws_source(ws_source, initial_stdin, true)
    }

    /// stdin にデータを追加する（interactive モード用）
    ///
    /// WaitingForInput 状態の際に呼び出し、次の step() で入力を再試行する。
    /// InputNumber の場合、改行（\n）付きで投入する必要がある。
    #[wasm_bindgen(js_name = "provideStdin")]
    pub fn provide_stdin(&mut self, data: &str) {
        self.vm.provide_stdin(data);
    }

    /// stdin のストリーム終端を通知する（interactive モード用）
    ///
    /// 以降、バッファが空の状態で入力命令に到達すると EOF として処理される。
    #[wasm_bindgen(js_name = "closeStdin")]
    pub fn close_stdin(&mut self) {
        self.vm.close_stdin();
    }

    /// 指定ステップ数だけ実行する
    ///
    /// 戻り値: { status: "suspended" | "complete" | "error" | "waiting_for_input", error?: string, inputType?: string }
    pub fn step(&mut self, budget: u32) -> JsVmStepResult {
        let result = self.vm.step(budget as usize);

        let vm_result = match result {
            StepResult::Suspended => VmStepResult {
                status: "suspended".to_string(),
                error: None,
                input_type: None,
            },
            StepResult::Complete => VmStepResult {
                status: "complete".to_string(),
                error: None,
                input_type: None,
            },
            StepResult::Error(e) => VmStepResult {
                status: "error".to_string(),
                error: Some(format!("{:?}", e)),
                input_type: None,
            },
            StepResult::WaitingForInput(input_type) => VmStepResult {
                status: "waiting_for_input".to_string(),
                error: None,
                input_type: Some(match input_type {
                    InputWaitType::Char => "char".to_string(),
                    InputWaitType::Number => "number".to_string(),
                }),
            },
        };

        let js: JsValue = serde_wasm_bindgen::to_value(&vm_result).unwrap();
        js.into()
    }

    /// 現在のプログラムカウンタ（命令インデックス）
    pub fn pc(&self) -> usize {
        self.vm.pc()
    }

    /// 総実行命令数
    pub fn total_steps(&self) -> usize {
        self.vm.total_steps()
    }

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool {
        self.vm.is_complete()
    }

    /// データスタックの現在の内容
    ///
    /// 戻り値: number[] (i64 → JS number に変換。53bit 超は精度が落ちる)
    pub fn get_stack(&self) -> JsNumberArray {
        let stack: Vec<f64> = self.vm.data_stack().iter().map(|&v| v as f64).collect();
        let js: JsValue = serde_wasm_bindgen::to_value(&stack).unwrap();
        js.into()
    }

    /// ヒープの現在の内容
    ///
    /// 戻り値: { [address: string]: number }
    pub fn get_heap(&self) -> JsNumberRecord {
        let heap: std::collections::BTreeMap<String, f64> = self
            .vm
            .heap()
            .iter()
            .map(|(k, v)| (k.to_string(), *v as f64))
            .collect();
        let js: JsValue = serde_wasm_bindgen::to_value(&heap).unwrap();
        js.into()
    }

    /// コールスタックの深さ
    pub fn call_stack_depth(&self) -> usize {
        self.vm.call_stack_depth()
    }

    /// 標準出力バッファの内容を取得しクリアする
    pub fn flush_stdout(&mut self) -> String {
        let mut buf = self.stdout_buffer.borrow_mut();
        let text = String::from_utf8_lossy(&buf).to_string();
        buf.clear();
        text
    }

    /// トレース情報を取得
    ///
    /// 戻り値: { [key: string]: number }
    pub fn get_traced(&self) -> JsNumberRecord {
        let traced: std::collections::BTreeMap<String, f64> = self
            .vm
            .traced
            .iter()
            .map(|(k, v)| (k.to_string(), *v as f64))
            .collect();
        let js: JsValue = serde_wasm_bindgen::to_value(&traced).unwrap();
        js.into()
    }

    /// 現在の命令のニーモニック表現を取得（デバッグ用）
    pub fn current_instruction(&self) -> Option<String> {
        self.vm.current_instruction()
    }

    /// 命令列全体のニーモニック表現を取得
    pub fn disassemble(&self) -> JsStringArray {
        let instructions = self.vm.disassemble();
        let js: JsValue = serde_wasm_bindgen::to_value(&instructions).unwrap();
        js.into()
    }
}
