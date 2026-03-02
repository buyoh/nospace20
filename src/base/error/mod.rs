//! エラー型モジュール
//!
//! プロジェクト内で使用する全エラー型を集約する。
//! `NospaceError` が CLI や WASM API に伝達されるエンベロープ型。

pub mod compile_error;
pub mod interpret_error;
pub mod parse_error;
pub mod validation_error;
pub mod ws_error;

#[allow(unused_imports)]
pub use compile_error::{CompileError, CompileErrorKind};
pub use interpret_error::InterpretError;
pub use parse_error::CodeParseError;
pub use validation_error::ValidationError;
pub use ws_error::{WsParseError, WsRuntimeError};

/// コンパイルパイプラインのステージ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileStage {
    Tokenize,
    Parse,
    SemanticAnalysis,
    Optimization,
    WsCodeGeneration,
    NospaceExecution,
    WsExecution,
    Validation,
}

/// 統一エラー型
///
/// パイプラインの各ステージから発生するエラーを一つの型で表現する。
/// CLI や WASM API でのエラーハンドリングを統一的に行える。
#[derive(Debug)]
pub enum NospaceError {
    /// トークン化・構文解析・意味解析エラー（複数件）
    Parse(Vec<CodeParseError>),
    /// Whitespace コンパイルエラー
    Compile(CompileError),
    /// nospace インタプリタ実行エラー
    Interpret(InterpretError),
    /// Whitespace VM パースエラー
    WsParse(WsParseError),
    /// Whitespace VM 実行時エラー
    WsRuntime(WsRuntimeError),
    /// 設定バリデーションエラー
    Validation(ValidationError),
}

impl std::fmt::Display for NospaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(errors) => {
                for (i, e) in errors.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}", e)?;
                }
                Ok(())
            }
            Self::Compile(e) => write!(f, "{}", e),
            Self::Interpret(e) => write!(f, "{}", e),
            Self::WsParse(e) => write!(f, "{}", e),
            Self::WsRuntime(e) => write!(f, "{}", e),
            Self::Validation(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for NospaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(e) => Some(e),
            Self::Interpret(e) => Some(e),
            Self::WsParse(e) => Some(e),
            Self::WsRuntime(e) => Some(e),
            Self::Validation(e) => Some(e),
            // Vec<CodeParseError> は単一の source にならない
            Self::Parse(_) => None,
        }
    }
}

impl From<Vec<CodeParseError>> for NospaceError {
    fn from(errors: Vec<CodeParseError>) -> Self {
        Self::Parse(errors)
    }
}

impl From<CompileError> for NospaceError {
    fn from(e: CompileError) -> Self {
        Self::Compile(e)
    }
}

impl From<InterpretError> for NospaceError {
    fn from(e: InterpretError) -> Self {
        Self::Interpret(e)
    }
}

impl From<WsParseError> for NospaceError {
    fn from(e: WsParseError) -> Self {
        Self::WsParse(e)
    }
}

impl From<WsRuntimeError> for NospaceError {
    fn from(e: WsRuntimeError) -> Self {
        Self::WsRuntime(e)
    }
}

impl From<ValidationError> for NospaceError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e)
    }
}

#[cfg(test)]
mod tests;
