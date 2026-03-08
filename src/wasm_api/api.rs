//! Top-level WASM API: `compile`, `parse` and helper functions
//!
//! Each API calls processing commonized in the internal pipeline (`pipeline` module).
//!
//! `run()` function has been removed as it was replaced by NospaceVM (`WasmNospaceVM`).
//! For one-shot execution, use the `step()` loop of `WasmNospaceVM`.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::{compile_to_ws, CompileTarget, LanguageStd, WsCompileOptions, WsOutputFormat};

use super::pipeline;
use super::types::{
    CompileResultOk, JsCompileResult, JsOptPassArray, JsOptionsDefinition, JsParseResult,
    JsStdExtensionArray, ResultErr,
};

// ========================================
// Top-level API
// ========================================

/// Compile nospace source code.
/// Equivalent to CLI's `--mode=compile`.
///
/// - `std_extensions`: Array of extensions to enable (e.g., `["debug", "alloc"]`)
/// - `opt_passes`: Array of optimization passes to enable (e.g., `["all"]` or `["constant-folding", "dead-code"]`)
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

    // Parameter conversion
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

    let _language_std = match lang_std {
        "standard" => LanguageStd::Standard,
        _ => {
            let e = ResultErr::single_error(format!(
                "unsupported std: '{}' (use 'standard')",
                lang_std
            ));
            return serde_wasm_bindgen::to_value(&e).unwrap().into();
        }
    };

    // Parse + optimize
    let (scope, text_code, _) = match pipeline::analyze_and_optimize(source, opt_passes) {
        Ok(v) => v,
        Err(e) => return serde_wasm_bindgen::to_value(&e).unwrap().into(),
    };

    // Compile
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
            // Convert CompileError with position information
            let err_result = pipeline::convert_compile_error(&e, &text_code);
            serde_wasm_bindgen::to_value(&err_result).unwrap().into()
        }
    }
}

/// Perform only syntax checking on nospace source code.
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
// Helper functions and metadata
// ========================================

/// Return a list of available options
///
/// Get option values that can be specified in compile() or WasmWhitespaceVM.
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
        language_stds: vec!["standard"],
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
