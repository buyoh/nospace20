//! Common pipeline, parameter parsers, and error conversion
//!
//! Commonalizes the pipeline duplicated in 4 places:
//! `parse_to_tokens → parse_to_tree → semantic_analyze`
//! Parameter parsers and error conversion are also consolidated here.

use wasm_bindgen::prelude::*;

use crate::{
    compile_source, CodeParseError, CompileError, NospaceError, OptimizationOptions, Scope,
    TextCode, WsCompileOptions, WsOutputFormat,
};

use crate::optimizer::PASS_NAMES;

use super::types::{JsOptPassArray, JsStdExtensionArray, ResultErr, WasmError};

// ========================================
// Parameter parsers
// ========================================

/// Parse `StdExtension[]` (JS array) and return enabled/disabled status for each extension
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
    let ext_list: Vec<String> = serde_wasm_bindgen::from_value(js_val)
        .map_err(|e| ResultErr::single_error(format!("invalid std_extensions: {}", e)))?;
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

/// Parse `OptPass[]` (JS array) and return `OptimizationOptions`
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
    let pass_list: Vec<String> = serde_wasm_bindgen::from_value(js_val)
        .map_err(|e| ResultErr::single_error(format!("invalid opt_passes: {}", e)))?;
    OptimizationOptions::from_pass_names(&pass_list).map_err(|pass| {
        ResultErr::single_error(format!(
            "unknown opt pass: '{}' (use {})",
            pass,
            PASS_NAMES
                .iter()
                .map(|n| format!("'{}'", n))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}

// ========================================
// Error conversion
// ========================================

/// Generate error detail string (equivalent to CLI output)
///
/// Example output:
/// ```text
/// line:7 column:10
///   (*next)[0] = tail;
///          ^
/// ```
fn format_error_details(text: &TextCode, line_0: usize, column_0: usize) -> String {
    let line_str = text.line(line_0);
    let line_1 = line_0 + 1;
    let col_1 = column_0 + 1;
    let prefix: String = line_str.chars().take(column_0).collect();
    let width = unicode_width::UnicodeWidthStr::width(prefix.as_str());
    format!(
        "line:{} column:{}\n{}\n{}^",
        line_1,
        col_1,
        line_str,
        " ".repeat(width)
    )
}

/// Convert `CompileError` to `ResultErr`
///
/// `CompileError` has position information (`SourceLocation`),
/// so we use `TextCode` to convert it to line and column numbers.
pub(super) fn convert_compile_error(error: &CompileError, text: &TextCode) -> ResultErr {
    let (line, column, details) = if let Some(loc) = &error.location {
        let (l, c) = text.char_index_to_line(loc.start);
        let details = format_error_details(text, l, c);
        (Some(l + 1), Some(c + 1), Some(details))
    } else {
        (None, None, None)
    };
    ResultErr {
        success: false,
        errors: vec![WasmError {
            message: format!("{}", error),
            line,
            column,
            details,
        }],
    }
}

/// Convert `CodeParseError[]` to `ResultErr`
pub(super) fn convert_errors(errors: &[CodeParseError], text: &TextCode) -> ResultErr {
    let wasm_errors: Vec<WasmError> = errors
        .iter()
        .map(|e| {
            let (line, column, details) = if let Some(p) = e.code_pointer {
                let (l, c) = text.char_index_to_line(p);
                let details = format_error_details(text, l, c);
                // NOTE: char_index_to_line is 0-indexed. Convert to 1-indexed for user-facing display.
                (Some(l + 1), Some(c + 1), Some(details))
            } else {
                (None, None, None)
            };
            WasmError {
                message: e.message.to_string(),
                line,
                column,
                details,
            }
        })
        .collect();

    ResultErr {
        success: false,
        errors: wasm_errors,
    }
}

// ========================================
// Common compilation pipeline
// ========================================

/// Run token analysis, syntax analysis and semantic analysis without optimization (for `parse`)
///
/// The pipeline itself is `crate::analyze_source`, shared with the CLI and tests;
/// this only converts errors into `ResultErr` with positions.
pub(super) fn analyze_for_parse(source: &str) -> Result<(Scope, TextCode<'_>), ResultErr> {
    let text_code = TextCode::new(source);
    let scope = crate::analyze_source(source, &OptimizationOptions::none())
        .map_err(|e| convert_errors(&e, &text_code))?;
    Ok((scope, text_code))
}

/// Compile nospace source to Whitespace (common pipeline + optimization + code generation)
///
/// Used by both `compile` and `WasmWhitespaceVM::new`. The pipeline itself is
/// `crate::compile_source`, shared with the CLI and tests; this only parses the
/// optimization passes and converts errors into `ResultErr` with positions.
pub(super) fn compile_nospace(
    source: &str,
    opt_passes: Option<JsOptPassArray>,
    debug_ext: bool,
    alloc_ext: bool,
    output_format: WsOutputFormat,
) -> Result<String, ResultErr> {
    // バグ修正（12-wasm-compile-opt-mismatch）: 最適化オプションを WsCompileOptions に渡さないと
    // peephole（WsProgram レベルの最適化）が適用されず CLI と出力が異なる。解析と同じ値を渡す。
    // NOTE: 最適化パス名の検証は解析より先に行う。ソースの誤りと不正なパス名が同時にある場合は
    // パス名のエラーを返す（以前は解析のエラーを返していた。VERSION.md に記載）
    let options = WsCompileOptions {
        debug_ext,
        alloc_ext,
        output_format,
        optimization: parse_opt_passes(opt_passes)?,
    };
    let text_code = TextCode::new(source);
    compile_source(source, &options).map_err(|e| match e {
        NospaceError::Parse(errors) => convert_errors(&errors, &text_code),
        NospaceError::Compile(error) => convert_compile_error(&error, &text_code),
        other => ResultErr::single_error(other.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextCode;

    #[test]
    fn test_format_error_details_ascii() {
        let source = "hello world\nfoo bar baz\n";
        let text = TextCode::new(source);
        // line 1 (0-indexed), column 4 (0-indexed) → "bar"
        let result = format_error_details(&text, 1, 4);
        assert_eq!(result, "line:2 column:5\nfoo bar baz\n    ^");
    }

    #[test]
    fn test_format_error_details_first_column() {
        let source = "abcde\n";
        let text = TextCode::new(source);
        let result = format_error_details(&text, 0, 0);
        assert_eq!(result, "line:1 column:1\nabcde\n^");
    }

    #[test]
    fn test_convert_errors_has_details() {
        use crate::CodeParseError;
        // "int x = ;\n" のようなソース: エラー位置 8
        let source = "int x = ;\n";
        let text = TextCode::new(source);
        let errors = vec![CodeParseError::new(Some(8), "unexpected token")];
        let result = convert_errors(&errors, &text);
        assert!(!result.success);
        assert_eq!(result.errors.len(), 1);
        let err = &result.errors[0];
        assert_eq!(err.line, Some(1));
        assert_eq!(err.column, Some(9));
        let details = err.details.as_ref().expect("details should be Some");
        assert!(details.contains("line:1 column:9"), "details={}", details);
        assert!(details.contains("int x = ;"), "details={}", details);
        assert!(details.contains('^'), "details={}", details);
    }

    #[test]
    fn test_convert_errors_no_pointer_has_no_details() {
        use crate::CodeParseError;
        let source = "anything\n";
        let text = TextCode::new(source);
        let errors = vec![CodeParseError::new(None, "some error")];
        let result = convert_errors(&errors, &text);
        let err = &result.errors[0];
        assert!(err.details.is_none());
    }
}
