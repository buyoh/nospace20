//! Common pipeline, parameter parsers, and error conversion
//!
//! Commonalizes the pipeline duplicated in 4 places:
//! `parse_to_tokens → parse_to_tree → semantic_analyze`
//! Parameter parsers and error conversion are also consolidated here.

use wasm_bindgen::prelude::*;

use crate::{
    optimize, parse_to_tokens, parse_to_tree, semantic_analyze, CodeParseError, CompileError,
    OptimizationOptions, Scope, TextCode,
};

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

/// Parse nospace source with token analysis, syntax analysis, and semantic analysis (common pipeline)
///
/// Consolidates `parse_to_tokens → parse_to_tree → semantic_analyze` duplicated in 4 places:
/// `run`, `compile`, `parse`, `WasmWhitespaceVM::new`.
pub(super) fn analyze_source(source: &str) -> Result<(Scope, TextCode<'_>), ResultErr> {
    let text_code = TextCode::new(source);
    let source_string = source.to_string();

    let tokens = parse_to_tokens(&source_string).map_err(|e| convert_errors(&e, &text_code))?;
    let tree = parse_to_tree(&tokens).map_err(|e| convert_errors(&e, &text_code))?;
    let scope = semantic_analyze(&tree).map_err(|e| convert_errors(&e, &text_code))?;

    Ok((scope, text_code))
}

/// Common pipeline + apply optimization
///
/// "Analysis + optimization" flow used by both `run` and `compile` functions.
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
