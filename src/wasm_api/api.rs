//! トップレベル WASM API: `compile`, `parse` およびヘルパー関数
//!
//! 各 API は内部パイプライン（`pipeline` モジュール）で共通化された処理を呼び出す。
//!
//! `run()` 関数は NospaceVM (`WasmNospaceVM`) に置き換えられたため削除済み。
//! ワンショット実行が必要な場合は `WasmNospaceVM` の `step()` ループを使用する。

use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::{
    compile_to_ws, CompileTarget, LanguageStd,
    WsCompileOptions, WsOutputFormat,
};

use super::pipeline;
use super::types::{
    CompileResultOk, JsCompileResult, JsOptPassArray, JsOptionsDefinition, JsParseResult,
    JsStdExtensionArray, ResultErr,
};

// ========================================
// トップレベル API
// ========================================

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
