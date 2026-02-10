//! WebAssembly 公開 API
//!
//! CLI と同等の機能を JavaScript から呼び出し可能にする。
//! `wasm` feature が有効な場合のみコンパイルされる。

use serde::Serialize;
use wasm_bindgen::prelude::*;

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use crate::{
    compile_to_whitespace, compile_to_whitespace_debug, interpret_func_with_io, parse_to_tokens,
    parse_to_tree, syntactic_analyze, CodeParseError, CompileTarget, LanguageStd, TextCode,
};
use crate::whitespace::{WhitespaceVM, StepResult, RuntimeError};

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
pub fn run(source: &str, stdin: &str, debug: bool) -> JsValue {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    // 字句解析
    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text),
    };

    // 構文解析
    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text),
    };

    // 意味解析
    let scope = match syntactic_analyze(&statements) {
        Ok(a) => a,
        Err(errors) => return convert_errors(&errors, &text),
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
    serde_wasm_bindgen::to_value(&result).unwrap()
}

/// nospace ソースコードをコンパイルする。
/// CLI の `--mode=compile` に相当。
#[wasm_bindgen]
pub fn compile(source: &str, target: &str, lang_std: &str) -> JsValue {
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
            return serde_wasm_bindgen::to_value(&result).unwrap();
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
            return serde_wasm_bindgen::to_value(&result).unwrap();
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
        return serde_wasm_bindgen::to_value(&result).unwrap();
    }

    // 解析
    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text),
    };
    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text),
    };
    let scope = match syntactic_analyze(&statements) {
        Ok(a) => a,
        Err(errors) => return convert_errors(&errors, &text),
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
            serde_wasm_bindgen::to_value(&result).unwrap()
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
            serde_wasm_bindgen::to_value(&result).unwrap()
        }
    }
}

/// nospace ソースコードの構文チェックのみ行う。
#[wasm_bindgen]
pub fn parse(source: &str) -> JsValue {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text),
    };

    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text),
    };

    match syntactic_analyze(&statements) {
        Ok(_) => {
            let result = serde_json::json!({ "success": true });

// ========================================
// Whitespace VM のステップ実行 API
// ========================================

/// Whitespace VM の実行結果型
#[derive(Serialize)]
struct VmStepResult {
    status: String, // "suspended" | "complete" | "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
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
    pub fn new(nospace_source: &str, stdin: &str) -> Result<WasmWhitespaceVM, JsValue> {
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

        Self::from_whitespace(&ws_source, stdin)
    }

    /// Whitespace ソースコードから直接 VM を構築する
    #[wasm_bindgen(js_name = "fromWhitespace")]
    pub fn from_whitespace(ws_source: &str, stdin: &str) -> Result<WasmWhitespaceVM, JsValue> {
        // VM を構築
        let vm_result = WhitespaceVM::from_source(ws_source);
        let vm = match vm_result {
            Ok(v) => v,
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

    /// 指定ステップ数だけ実行する
    ///
    /// 戻り値: { status: "suspended" | "complete" | "error", error?: string }
    pub fn step(&mut self, budget: u32) -> JsValue {
        let result = self.vm.step(budget as usize);

        let vm_result = match result {
            StepResult::Suspended => VmStepResult {
                status: "suspended".to_string(),
                error: None,
            },
            StepResult::Complete => VmStepResult {
                status: "complete".to_string(),
                error: None,
            },
            StepResult::Error(e) => VmStepResult {
                status: "error".to_string(),
                error: Some(format!("{:?}", e)),
            },
        };

        serde_wasm_bindgen::to_value(&vm_result).unwrap()
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
    pub fn get_stack(&self) -> JsValue {
        let stack: Vec<f64> = self.vm.data_stack().iter().map(|&v| v as f64).collect();
        serde_wasm_bindgen::to_value(&stack).unwrap()
    }

    /// ヒープの現在の内容
    ///
    /// 戻り値: { [address: string]: number }
    pub fn get_heap(&self) -> JsValue {
        let heap: std::collections::BTreeMap<String, f64> = self
            .vm
            .heap()
            .iter()
            .map(|(k, v)| (k.to_string(), *v as f64))
            .collect();
        serde_wasm_bindgen::to_value(&heap).unwrap()
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
    pub fn get_traced(&self) -> JsValue {
        let traced: std::collections::BTreeMap<String, f64> = self
            .vm
            .traced
            .iter()
            .map(|(k, v)| (k.to_string(), *v as f64))
            .collect();
        serde_wasm_bindgen::to_value(&traced).unwrap()
    }

    /// 現在の命令のニーモニック表現を取得（デバッグ用）
    pub fn current_instruction(&self) -> Option<String> {
        self.vm.current_instruction()
    }

    /// 命令列全体のニーモニック表現を取得
    pub fn disassemble(&self) -> JsValue {
        let instructions = self.vm.disassemble();
        serde_wasm_bindgen::to_value(&instructions).unwrap()
    }
}

/// nospace ソースコードを Whitespace にコンパイル（ヘルパー関数）
#[wasm_bindgen]
pub fn compile_to_whitespace_string(source: &str) -> JsValue {
    compile(source, "ws", "ws")
}

/// nospace ソースコードをニーモニックにコンパイル（ヘルパー関数）
#[wasm_bindgen]
pub fn compile_to_mnemonic_string(source: &str) -> JsValue {
    compile(source, "mnemonic", "ws")
}
            serde_wasm_bindgen::to_value(&result).unwrap()
        }
        Err(errors) => convert_errors(&errors, &text),
    }
}
