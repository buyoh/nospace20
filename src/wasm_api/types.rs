//! TypeScript 型定義・Serde 結果構造体
//!
//! wasm_bindgen の `extern "C"` 型と、WASM API が返す Serde シリアライズ構造体を定義する。

use serde::Serialize;
use wasm_bindgen::prelude::*;

// ========================================
// TypeScript 型定義
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

/** コンパイルターゲット */
type CompileTarget = "ws" | "mnemonic";

/** 言語サブセット */
type LanguageStd = "standard" | "ws";

/** ターゲット拡張 */
type StdExtension = "debug" | "alloc";

/** 利用可能な最適化パス */
type OptPass = "all" | "condition-opt" | "geti-opt" | "constant-folding" | "dead-code";

/** 利用可能なオプション定義 */
interface OptionsDefinition {
    readonly compileTargets: readonly CompileTarget[];
    readonly languageStds: readonly LanguageStd[];
    readonly stdExtensions: readonly StdExtension[];
    readonly optPasses: readonly OptPass[];
}
"#;

// ========================================
// extern "C" 型（wasm_bindgen の外部型）
// ========================================

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

    #[wasm_bindgen(typescript_type = "OptionsDefinition")]
    pub type JsOptionsDefinition;

    #[wasm_bindgen(typescript_type = "StdExtension[]")]
    pub type JsStdExtensionArray;

    #[wasm_bindgen(typescript_type = "OptPass[]")]
    pub type JsOptPassArray;
}

// ========================================
// Serde 結果構造体
// ========================================

/// 実行成功時のレスポンス
#[derive(Serialize)]
pub struct RunResultOk {
    pub success: bool, // always true
    #[serde(rename = "returnValue")]
    pub return_value: Option<i64>,
    pub stdout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<std::collections::BTreeMap<String, String>>,
}

/// エラーレスポンス（構文エラー等）
#[derive(Serialize)]
pub struct ResultErr {
    pub success: bool, // always false
    pub errors: Vec<WasmError>,
}

impl ResultErr {
    /// 単一エラーメッセージから ResultErr を構築する
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

/// コンパイル成功時のレスポンス
#[derive(Serialize)]
pub struct CompileResultOk {
    pub success: bool, // always true
    pub output: String,
}

/// WASM API が返すエラーの詳細
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

/// Whitespace VM のステップ実行結果
#[derive(Serialize)]
pub struct VmStepResult {
    pub status: String, // "suspended" | "complete" | "error" | "waiting_for_input"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inputType")]
    pub input_type: Option<String>,
}
