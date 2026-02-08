//! WebAssembly 公開 API
//!
//! CLI と同等の機能を JavaScript から呼び出し可能にする。
//! `wasm` feature が有効な場合のみコンパイルされる。

use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::{
    compile_to_whitespace, compile_to_whitespace_debug,
    interpret_func_with_io, parse_to_tokens, parse_to_tree,
    syntactic_analyze, CodeParseError, CompileTarget, LanguageStd,
    TextCode,
};

#[derive(Serialize)]
struct RunResultOk {
    success: bool,           // always true
    #[serde(rename = "returnValue")]
    return_value: Option<i64>,
    stdout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct ResultErr {
    success: bool,           // always false
    errors: Vec<WasmError>,
}

#[derive(Serialize)]
struct CompileResultOk {
    success: bool,           // always true
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
    let wasm_errors: Vec<WasmError> = errors.iter().map(|e| {
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
    }).collect();

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
        Some(traced.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    } else {
        None
    };

    // interpret_func_with_io は戻り値を返さないため None とする
    // TODO: interpret_with_io で戻り値も取得できるようにする
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
                message: format!(
                    "target='{}' requires std='ws'",
                    target
                ),
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
            serde_wasm_bindgen::to_value(&result).unwrap()
        }
        Err(errors) => convert_errors(&errors, &text),
    }
}
