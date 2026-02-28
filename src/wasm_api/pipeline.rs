//! 共通パイプライン・パラメータパーサ・エラー変換
//!
//! `parse_to_tokens → parse_to_tree → syntactic_analyze` の4箇所で重複していた
//! パイプラインを共通化し、パラメータパーサとエラー変換もここに集約する。

use wasm_bindgen::prelude::*;

use crate::{
    optimize, parse_to_tokens, parse_to_tree, syntactic_analyze, CodeParseError,
    OptimizationOptions, Scope, TextCode,
};

use super::types::{JsOptPassArray, JsStdExtensionArray, ResultErr, WasmError};

// ========================================
// パラメータパーサ
// ========================================

/// `StdExtension[]` (JS 配列) をパースし、各拡張の有効/無効を返す
pub(super) fn parse_std_extensions(
    extensions: Option<JsStdExtensionArray>,
) -> Result<(bool, bool), ResultErr> {
    let js_val: JsValue = match extensions {
        Some(v) => v.into(),
        None => return Ok((false, false)),
    };
    if js_val.is_undefined() || js_val.is_null() {
        return Ok((false, false));
    }
    let ext_list: Vec<String> =
        serde_wasm_bindgen::from_value(js_val).map_err(|e| ResultErr::single_error(format!("invalid std_extensions: {}", e)))?;
    let mut debug_ext = false;
    let mut alloc_ext = false;
    for ext in &ext_list {
        match ext.as_str() {
            "debug" => debug_ext = true,
            "alloc" => alloc_ext = true,
            _ => {
                return Err(ResultErr::single_error(format!(
                    "unknown std extension: '{}' (use 'debug' or 'alloc')",
                    ext
                )));
            }
        }
    }
    Ok((debug_ext, alloc_ext))
}

/// `OptPass[]` (JS 配列) をパースし、`OptimizationOptions` を返す
pub(super) fn parse_opt_passes(
    passes: Option<JsOptPassArray>,
) -> Result<OptimizationOptions, ResultErr> {
    let js_val: JsValue = match passes {
        Some(v) => v.into(),
        None => return Ok(OptimizationOptions::none()),
    };
    if js_val.is_undefined() || js_val.is_null() {
        return Ok(OptimizationOptions::none());
    }
    let pass_list: Vec<String> =
        serde_wasm_bindgen::from_value(js_val).map_err(|e| ResultErr::single_error(format!("invalid opt_passes: {}", e)))?;
    if pass_list.is_empty() {
        return Ok(OptimizationOptions::none());
    }
    if pass_list.iter().any(|p| p == "all") {
        return Ok(OptimizationOptions::all());
    }
    let mut opts = OptimizationOptions::none();
    for pass in &pass_list {
        match pass.as_str() {
            "condition-opt" => opts.condition_opt = true,
            "geti-opt" => opts.geti_opt = true,
            "constant-folding" => opts.constant_folding = true,
            "dead-code" => opts.dead_code = true,
            _ => {
                return Err(ResultErr::single_error(format!(
                    "unknown opt pass: '{}' (use 'all', 'condition-opt', 'geti-opt', 'constant-folding', 'dead-code')",
                    pass
                )));
            }
        }
    }
    Ok(opts)
}

// ========================================
// エラー変換
// ========================================

/// `CodeParseError[]` を `ResultErr` に変換する
pub(super) fn convert_errors(errors: &[CodeParseError], text: &TextCode) -> ResultErr {
    let wasm_errors: Vec<WasmError> = errors
        .iter()
        .map(|e| {
            let (line, column) = if let Some(p) = e.code_pointer {
                let (l, c) = text.char_index_to_line(p);
                // NOTE: char_index_to_line は0-indexed。ユーザー向けには1-indexedにする。
                (Some(l + 1), Some(c + 1))
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

    ResultErr {
        success: false,
        errors: wasm_errors,
    }
}

// ========================================
// 共通コンパイルパイプライン
// ========================================

/// nospace ソースをトークン解析・構文解析・意味解析する（共通パイプライン）
///
/// `run`, `compile`, `parse`, `WasmWhitespaceVM::new` の4箇所で重複していた
/// `parse_to_tokens → parse_to_tree → syntactic_analyze` を一箇所に集約。
pub(super) fn analyze_source(source: &str) -> Result<(Scope, TextCode<'_>), ResultErr> {
    let text_code = TextCode::new(source);
    let source_string = source.to_string();

    let tokens = parse_to_tokens(&source_string).map_err(|e| convert_errors(&e, &text_code))?;
    let tree = parse_to_tree(&tokens).map_err(|e| convert_errors(&e, &text_code))?;
    let scope = syntactic_analyze(&tree).map_err(|e| convert_errors(&e, &text_code))?;

    Ok((scope, text_code))
}

/// 共通パイプライン + 最適化適用
///
/// `run`, `compile` の両関数で使われる「解析 + 最適化」のフロー。
pub(super) fn analyze_and_optimize(
    source: &str,
    opt_passes: Option<JsOptPassArray>,
) -> Result<(Scope, TextCode<'_>, OptimizationOptions), ResultErr> {
    let (mut scope, text_code) = analyze_source(source)?;
    let opt_options = parse_opt_passes(opt_passes)?;
    if opt_options.any_enabled() {
        optimize(&mut scope, &opt_options);
    }
    Ok((scope, text_code, opt_options))
}
