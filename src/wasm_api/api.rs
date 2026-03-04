//! トップレベル WASM API: `run`, `compile`, `parse` およびヘルパー関数
//!
//! 各 API は内部パイプライン（`pipeline` モジュール）で共通化された処理を呼び出す。

use serde::Serialize;
use wasm_bindgen::prelude::*;

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use crate::{
    compile_to_ws, interpret_with_env, CompileTarget, Environment, EnvironmentConfig, LanguageStd,
    WsCompileOptions, WsOutputFormat,
};

use super::pipeline;
use super::types::{
    CompileResultOk, JsCompileResult, JsOptPassArray, JsOptionsDefinition, JsParseResult,
    JsRunResult, JsStdExtensionArray, ResultErr, RunResultOk,
};
use super::whitespace_vm::SharedWriter;

// ========================================
// トップレベル API
// ========================================

/// nospace ソースコードを解析・実行する。
/// CLI の `--mode=run` に相当。
///
/// - `ignore_debug`: デバッグ用組み込み関数（__assert, __trace 等）を無視する（CLI の `--ignore-debug` 相当）
/// - `opt_passes`: 有効にする最適化パスの配列（例: `["all"]` または `["constant-folding", "dead-code"]`）
#[wasm_bindgen]
pub fn run(
    source: &str,
    stdin: &str,
    debug: bool,
    ignore_debug: Option<bool>,
    opt_passes: Option<JsOptPassArray>,
) -> JsRunResult {
    let (scope, _text_code, _) = match pipeline::analyze_and_optimize(source, opt_passes) {
        Ok(v) => v,
        Err(e) => return serde_wasm_bindgen::to_value(&e).unwrap().into(),
    };

    // 実行
    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(
        stdin.as_bytes().to_vec(),
    )));
    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let stdout_clone = Rc::clone(&stdout_buf);
    let mut config = EnvironmentConfig::with_max_expression_count(100000);
    config.ignore_debug = ignore_debug.unwrap_or(false);
    let mut env =
        Environment::new_with_config(stdin_cursor, Box::new(SharedWriter(stdout_clone)), config);
    if let Err(e) = interpret_with_env(&mut env, &scope) {
        let err_result = ResultErr::single_error(format!("{}", e));
        return serde_wasm_bindgen::to_value(&err_result).unwrap().into();
    }
    env.flush();

    let stdout_vec = stdout_buf.borrow().clone();
    let stdout_str = String::from_utf8(stdout_vec).unwrap_or_default();

    // trace を String キーに変換 (JSON 互換)
    let trace = if debug {
        Some(
            env.traced
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
///
/// - `std_extensions`: 有効にする拡張の配列（例: `["debug", "alloc"]`）
/// - `opt_passes`: 有効にする最適化パスの配列（例: `["all"]` または `["constant-folding", "dead-code"]`）
#[wasm_bindgen]
pub fn compile(
    source: &str,
    target: &str,
    lang_std: &str,
    std_extensions: Option<JsStdExtensionArray>,
    opt_passes: Option<JsOptPassArray>,
) -> JsCompileResult {
    let (debug_ext, alloc_ext) = match pipeline::parse_std_extensions(std_extensions) {
        Ok(v) => v,
        Err(e) => return serde_wasm_bindgen::to_value(&e).unwrap().into(),
    };

    // パラメータ変換
    let compile_target = match target {
        "ws" => CompileTarget::Ws,
        "mnemonic" => CompileTarget::Mnemonic,
        _ => {
            let e = ResultErr::single_error(format!(
                "unsupported target: '{}' (use 'ws' or 'mnemonic')",
                target
            ));
            return serde_wasm_bindgen::to_value(&e).unwrap().into();
        }
    };

    let language_std = match lang_std {
        "ws" => LanguageStd::Ws,
        "standard" => LanguageStd::Standard,
        _ => {
            let e = ResultErr::single_error(format!(
                "unsupported std: '{}' (use 'standard' or 'ws')",
                lang_std
            ));
            return serde_wasm_bindgen::to_value(&e).unwrap().into();
        }
    };

    // バリデーション
    if matches!(compile_target, CompileTarget::Ws | CompileTarget::Mnemonic)
        && language_std != LanguageStd::Ws
    {
        let e = ResultErr::single_error(format!("target='{}' requires std='ws'", target));
        return serde_wasm_bindgen::to_value(&e).unwrap().into();
    }

    // 解析 + 最適化
    let (scope, text_code, _) = match pipeline::analyze_and_optimize(source, opt_passes) {
        Ok(v) => v,
        Err(e) => return serde_wasm_bindgen::to_value(&e).unwrap().into(),
    };

    // コンパイル
    let output_format = match compile_target {
        CompileTarget::Ws => WsOutputFormat::Whitespace,
        CompileTarget::Mnemonic => WsOutputFormat::Mnemonic,
        _ => unreachable!(),
    };
    let ws_options = WsCompileOptions {
        debug_ext,
        alloc_ext,
        output_format,
        ..Default::default()
    };
    let compiled = compile_to_ws(&scope, &ws_options);

    match compiled {
        Ok(output) => {
            let result = CompileResultOk {
                success: true,
                output,
            };
            let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
            js.into()
        }
        Err(e) => {
            // CompileError を位置情報付きで変換
            let err_result = pipeline::convert_compile_error(&e, &text_code);
            serde_wasm_bindgen::to_value(&err_result).unwrap().into()
        }
    }
}

/// nospace ソースコードの構文チェックのみ行う。
#[wasm_bindgen]
pub fn parse(source: &str) -> JsParseResult {
    match pipeline::analyze_source(source) {
        Ok(_) => {
            #[derive(Serialize)]
            struct ParseResultOk {
                success: bool,
            }
            let result = ParseResultOk { success: true };
            let js: JsValue = serde_wasm_bindgen::to_value(&result).unwrap();
            js.into()
        }
        Err(e) => serde_wasm_bindgen::to_value(&e).unwrap().into(),
    }
}

// ========================================
// ヘルパー関数・メタデータ
// ========================================

/// nospace ソースコードを Whitespace にコンパイル（ヘルパー関数）
#[wasm_bindgen]
pub fn compile_to_whitespace_string(source: &str) -> JsCompileResult {
    compile(source, "ws", "ws", None, None)
}

/// nospace ソースコードをニーモニックにコンパイル（ヘルパー関数）
#[wasm_bindgen]
pub fn compile_to_mnemonic_string(source: &str) -> JsCompileResult {
    compile(source, "mnemonic", "ws", None, None)
}

/// 利用可能なオプションの一覧を返す
///
/// compile() や WasmWhitespaceVM で指定可能なオプション値を取得できる。
#[wasm_bindgen(js_name = "getOptions")]
pub fn get_options() -> JsOptionsDefinition {
    #[derive(Serialize)]
    struct OptionsDefinition {
        #[serde(rename = "compileTargets")]
        compile_targets: Vec<&'static str>,
        #[serde(rename = "languageStds")]
        language_stds: Vec<&'static str>,
        #[serde(rename = "stdExtensions")]
        std_extensions: Vec<&'static str>,
        #[serde(rename = "optPasses")]
        opt_passes: Vec<&'static str>,
    }

    let options = OptionsDefinition {
        compile_targets: vec!["ws", "mnemonic"],
        language_stds: vec!["standard", "ws"],
        std_extensions: vec!["debug", "alloc"],
        opt_passes: vec![
            "all",
            "condition-opt",
            "geti-opt",
            "constant-folding",
            "dead-code",
        ],
    };
    let js: JsValue = serde_wasm_bindgen::to_value(&options).unwrap();
    js.into()
}
