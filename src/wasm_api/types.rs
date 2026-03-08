//! TypeScript type definitions and Serde result structures
//!
//! Defines wasm_bindgen `extern "C"` types and Serde-serializable structures returned by WASM API.

use serde::Serialize;
use wasm_bindgen::prelude::*;

// ========================================
// TypeScript type definitions
// ========================================

#[wasm_bindgen(typescript_custom_section)]
pub const TS_TYPES: &str = r#"
interface WasmError {
    message: string;
    line?: number;
    column?: number;
    details?: string;
}

interface ResultErr {
    success: false;
    errors: WasmError[];
}

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

/** Compile target */
type CompileTarget = "ws" | "mnemonic";

/** Language subset */
type LanguageStd = "standard" | "ws";

/** Target extensions */
type StdExtension = "debug" | "alloc";

/** Available optimization passes */
type OptPass = "all" | "condition-opt" | "geti-opt" | "constant-folding" | "dead-code";

/** Available options definition */
interface OptionsDefinition {
    readonly compileTargets: readonly CompileTarget[];
    readonly languageStds: readonly LanguageStd[];
    readonly stdExtensions: readonly StdExtension[];
    readonly optPasses: readonly OptPass[];
}
"#;

// ========================================
// extern "C" types (wasm_bindgen external types)
// ========================================

#[wasm_bindgen]
extern "C" {
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

    #[wasm_bindgen(typescript_type = "OptionsDefinition")]
    pub type JsOptionsDefinition;

    #[wasm_bindgen(typescript_type = "StdExtension[]")]
    pub type JsStdExtensionArray;

    #[wasm_bindgen(typescript_type = "OptPass[]")]
    pub type JsOptPassArray;
}

// ========================================
// Serde result structures
// ========================================

/// Error response (for syntax errors, etc.)
#[derive(Serialize)]
pub struct ResultErr {
    pub success: bool, // always false
    pub errors: Vec<WasmError>,
}

impl ResultErr {
    /// Construct ResultErr from a single error message
    pub fn single_error(message: String) -> Self {
        Self {
            success: false,
            errors: vec![WasmError {
                message,
                line: None,
                column: None,
                details: None,
            }],
        }
    }
}

/// Response on successful compilation
#[derive(Serialize)]
pub struct CompileResultOk {
    pub success: bool, // always true
    pub output: String,
}

/// Detailed error information returned by WASM API
#[derive(Serialize)]
pub struct WasmError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Step execution result of Whitespace VM
#[derive(Serialize)]
pub struct VmStepResult {
    pub status: String, // "suspended" | "complete" | "error" | "waiting_for_input"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inputType")]
    pub input_type: Option<String>,
}
