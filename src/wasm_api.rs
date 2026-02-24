//! WebAssembly 公開 API
//!
//! CLI と同等の機能を JavaScript から呼び出し可能にする。
//! `wasm` feature が有効な場合のみコンパイルされる。

use serde::Serialize;
use wasm_bindgen::prelude::*;

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use crate::whitespace::{InputWaitType, StepResult, WhitespaceVM};
use crate::{
    compile_to_whitespace, compile_to_whitespace_debug, compile_to_whitespace_debug_with_options,
    compile_to_whitespace_with_options, interpret_func_with_io, parse_to_tokens, parse_to_tree,
    syntactic_analyze, CodeParseError, CompileTarget, LanguageStd, TextCode,
};

// ========================================
// TypeScript 型定義
// ========================================

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &str = r#"
interface WasmError {
    message: string;
    line?: number;
    column?: number;
}

interface ResultErr {
    success: false;
    errors: WasmError[];
}

interface RunResultOk {
    success: true;
    returnValue: number | null;
    stdout: string;
    trace?: Record<string, string>;
}

type RunResult = RunResultOk | ResultErr;

interface CompileResultOk {
    success: true;
    output: string;
}

type CompileResult = CompileResultOk | ResultErr;

interface ParseResultOk {
    success: true;
}

type ParseResult = ParseResultOk | ResultErr;

interface VmStepResult {
    status: "suspended" | "complete" | "error" | "waiting_for_input";
    error?: string;
    inputType?: "char" | "number";
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "RunResult")]
    pub type JsRunResult;

    #[wasm_bindgen(typescript_type = "CompileResult")]
    pub type JsCompileResult;

    #[wasm_bindgen(typescript_type = "ParseResult")]
    pub type JsParseResult;

    #[wasm_bindgen(typescript_type = "VmStepResult")]
    pub type JsVmStepResult;

    #[wasm_bindgen(typescript_type = "number[]")]
    pub type JsNumberArray;

    #[wasm_bindgen(typescript_type = "Record<string, number>")]
    pub type JsNumberRecord;

    #[wasm_bindgen(typescript_type = "string[]")]
    pub type JsStringArray;
}

#[derive(Serialize)]
struct RunResultOk {
    success: bool, // always true
    #[serde(rename = "returnValue")]
    return_value: Option<i64>,
    stdout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct ResultErr {
    success: bool, // always false
    errors: Vec<WasmError>,
}

#[derive(Serialize)]
struct CompileResultOk {
    success: bool, // always true
    output: String,
}

#[derive(Serialize)]
struct WasmError {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}

fn convert_errors(errors: &[CodeParseError], text: &TextCode) -> JsValue {
    let wasm_errors: Vec<WasmError> = errors
        .iter()
        .map(|e| {
            let (line, column) = if let Some(p) = e.code_pointer {
                let (l, c) = text.char_index_to_line(p);
                (Some(l), Some(c))
            } else {
                (None, None)
            };
            WasmError {
                message: e.message.to_string(),
                line,
                column,
            }
        })
        .collect();

    let result = ResultErr {
        success: false,
        errors: wasm_errors,
    };
    serde_wasm_bindgen::to_value(&result).unwrap()
}

/// nospace ソースコードを解析・実行する。
/// CLI の `--mode=run` に相当。
#[wasm_bindgen]
pub fn run(source: &str, stdin: &str, debug: bool) -> JsRunResult {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    // 字句解析
    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };

    // 構文解析
    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };

    // 意味解析
    let scope = match syntactic_analyze(&statements) {
        Ok(a) => a,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };

    // 実行
    let (traced, stdout_str) = interpret_func_with_io(&scope, "main", stdin);

    // trace を String キーに変換 (JSON 互換)
    let trace = if debug {
        Some(
            traced
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    } else {
        None
    };

    let result = RunResultOk {
        success: true,
        return_value: None,
        stdout: stdout_str,
        trace,
    };
    let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
    js.into()
}

/// nospace ソースコードをコンパイルする。
/// CLI の `--mode=compile` に相当。
#[wasm_bindgen]
pub fn compile(source: &str, target: &str, lang_std: &str) -> JsCompileResult {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    // パラメータ変換
    let compile_target = match target {
        "ws" => CompileTarget::Ws,
        "mnemonic" => CompileTarget::Mnemonic,
        _ => {
            let result = ResultErr {
                success: false,
                errors: vec![WasmError {
                    message: format!("unsupported target: '{}' (use 'ws' or 'mnemonic')", target),
                    line: None,
                    column: None,
                }],
            };
            let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
            return js.into();
        }
    };

    let language_std = match lang_std {
        "ws" => LanguageStd::Ws,
        "standard" => LanguageStd::Standard,
        _ => {
            let result = ResultErr {
                success: false,
                errors: vec![WasmError {
                    message: format!("unsupported std: '{}' (use 'standard' or 'ws')", lang_std),
                    line: None,
                    column: None,
                }],
            };
            let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
            return js.into();
        }
    };

    // バリデーション
    if matches!(compile_target, CompileTarget::Ws | CompileTarget::Mnemonic)
        && language_std != LanguageStd::Ws
    {
        let result = ResultErr {
            success: false,
            errors: vec![WasmError {
                message: format!("target='{}' requires std='ws'", target),
                line: None,
                column: None,
            }],
        };
        let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
        return js.into();
    }

    // 解析
    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };
    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };
    let scope = match syntactic_analyze(&statements) {
        Ok(a) => a,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };

    // コンパイル
    let compiled = match compile_target {
        CompileTarget::Ws => compile_to_whitespace(&scope),
        CompileTarget::Mnemonic => compile_to_whitespace_debug(&scope),
        _ => unreachable!(),
    };

    match compiled {
        Ok(output) => {
            let result = CompileResultOk {
                success: true,
                output,
            };
            let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
            js.into()
        }
        Err(err) => {
            let result = ResultErr {
                success: false,
                errors: vec![WasmError {
                    message: err,
                    line: None,
                    column: None,
                }],
            };
            let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
            js.into()
        }
    }
}

/// nospace ソースコードの構文チェックのみ行う。
#[wasm_bindgen]
pub fn parse(source: &str) -> JsParseResult {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };

    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text).into(),
    };

    match syntactic_analyze(&statements) {
        Ok(_) => {
            #[derive(Serialize)]
            struct ParseResultOk {
                success: bool,
            }
            let result = ParseResultOk { success: true };
            let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
            js.into()
        }
        Err(errors) => convert_errors(&errors, &text).into(),
    }
}

// ========================================
// Whitespace VM のステップ実行 API
// ========================================

/// Whitespace VM の実行結果型
#[derive(Serialize)]
struct VmStepResult {
    status: String, // "suspended" | "complete" | "error" | "waiting_for_input"
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inputType")]
    input_type: Option<String>,
}

/// SharedWriter: Rc<RefCell<Vec<u8>>> をラップして Write トレイトを実装
struct SharedWriter(Rc<RefCell<Vec<u8>>>);

impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

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
    #[wasm_bindgen(constructor)]
    pub fn new(
        nospace_source: &str,
        stdin: &str,
        interactive: Option<bool>,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        let text = TextCode::new(nospace_source);
        let source_string = nospace_source.to_string();

        // パース・解析
        let tokens = match parse_to_tokens(&source_string) {
            Ok(t) => t,
            Err(errors) => return Err(convert_errors(&errors, &text)),
        };
        let statements = match parse_to_tree(&tokens) {
            Ok(s) => s,
            Err(errors) => return Err(convert_errors(&errors, &text)),
        };
        let scope = match syntactic_analyze(&statements) {
            Ok(a) => a,
            Err(errors) => return Err(convert_errors(&errors, &text)),
        };

        // コンパイル
        let ws_source = match compile_to_whitespace(&scope) {
            Ok(output) => output,
            Err(err) => {
                let result = ResultErr {
                    success: false,
                    errors: vec![WasmError {
                        message: err,
                        line: None,
                        column: None,
                    }],
                };
                return Err(serde_wasm_bindgen::to_value(&result).unwrap());
            }
        };

        if interactive.unwrap_or(false) {
            Self::from_whitespace_interactive(&ws_source, stdin)
        } else {
            Self::from_whitespace(&ws_source, stdin)
        }
    }

    /// Whitespace ソースコードから直接 VM を構築する
    #[wasm_bindgen(js_name = "fromWhitespace")]
    pub fn from_whitespace(ws_source: &str, stdin: &str) -> Result<WasmWhitespaceVM, JsValue> {
        // VM を構築
        let vm_result = WhitespaceVM::from_source(ws_source);
        let vm = match vm_result {
            Ok(v) => v.with_debug_ext(false),
            Err(e) => {
                let result = ResultErr {
                    success: false,
                    errors: vec![WasmError {
                        message: format!("Whitespace parse error: {:?}", e),
                        line: None,
                        column: None,
                    }],
                };
                return Err(serde_wasm_bindgen::to_value(&result).unwrap());
            }
        };

        // I/O セットアップ
        let stdin_buf = Box::new(std::io::BufReader::new(Cursor::new(
            stdin.as_bytes().to_vec(),
        )));

        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_clone = Rc::clone(&stdout_buf);

        let vm_with_io = vm.with_io(stdin_buf, Box::new(SharedWriter(stdout_clone)));

        Ok(WasmWhitespaceVM {
            vm: vm_with_io,
            stdout_buffer: stdout_buf,
        })
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
        let vm_result = WhitespaceVM::from_source(ws_source);
        let vm = match vm_result {
            Ok(v) => v.with_debug_ext(false).with_interactive_stdin(),
            Err(e) => {
                let result = ResultErr {
                    success: false,
                    errors: vec![WasmError {
                        message: format!("Whitespace parse error: {:?}", e),
                        line: None,
                        column: None,
                    }],
                };
                return Err(serde_wasm_bindgen::to_value(&result).unwrap());
            }
        };

        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_clone = Rc::clone(&stdout_buf);
        let mut vm_with_io = vm.with_stdout(Box::new(SharedWriter(stdout_clone)));

        // 初期データがあれば投入
        if !initial_stdin.is_empty() {
            vm_with_io.provide_stdin(initial_stdin);
        }

        Ok(WasmWhitespaceVM {
            vm: vm_with_io,
            stdout_buffer: stdout_buf,
        })
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

/// nospace ソースコードを Whitespace にコンパイル（ヘルパー関数）
#[wasm_bindgen]
pub fn compile_to_whitespace_string(source: &str) -> JsCompileResult {
    compile(source, "ws", "ws")
}

/// nospace ソースコードをニーモニックにコンパイル（ヘルパー関数）
#[wasm_bindgen]
pub fn compile_to_mnemonic_string(source: &str) -> JsCompileResult {
    compile(source, "mnemonic", "ws")
}
