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
mod tests {
    use super::*;

    #[test]
    fn test_nospace_error_display_parse() {
        let errors = vec![
            CodeParseError::new(Some(5), "unexpected token"),
            CodeParseError::new(None, "generic error"),
        ];
        let err = NospaceError::Parse(errors);
        let s = format!("{}", err);
        assert!(s.contains("unexpected token"));
        assert!(s.contains("generic error"));
    }

    #[test]
    fn test_nospace_error_display_compile() {
        let err = NospaceError::Compile(CompileError::new(CompileErrorKind::MainNotFound));
        assert_eq!(format!("{}", err), "__main function not found");
    }

    #[test]
    fn test_nospace_error_display_interpret() {
        let err = NospaceError::Interpret(InterpretError::FunctionNotFound("foo".to_string()));
        assert_eq!(format!("{}", err), "function 'foo' not found");
    }

    #[test]
    fn test_nospace_error_display_ws_runtime() {
        let err = NospaceError::WsRuntime(WsRuntimeError::StackUnderflow);
        assert_eq!(format!("{}", err), "stack underflow");
    }

    #[test]
    fn test_nospace_error_display_ws_parse() {
        let err = NospaceError::WsParse(WsParseError::InvalidImp { position: 3 });
        assert_eq!(format!("{}", err), "invalid IMP at position 3");
    }

    #[test]
    fn test_nospace_error_is_std_error() {
        let err = NospaceError::WsRuntime(WsRuntimeError::DivisionByZero);
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_nospace_error_from_vec_code_parse_error() {
        let errors = vec![CodeParseError::new(None, "err")];
        let err: NospaceError = errors.into();
        assert!(matches!(err, NospaceError::Parse(_)));
    }

    #[test]
    fn test_nospace_error_from_compile_error() {
        let e = CompileError::new(CompileErrorKind::MainNotFound);
        let err: NospaceError = e.into();
        assert!(matches!(err, NospaceError::Compile(_)));
    }

    #[test]
    fn test_nospace_error_from_interpret_error() {
        let e = InterpretError::FunctionNotFound("x".to_string());
        let err: NospaceError = e.into();
        assert!(matches!(err, NospaceError::Interpret(_)));
    }

    #[test]
    fn test_nospace_error_from_ws_parse_error() {
        let e = WsParseError::InvalidImp { position: 0 };
        let err: NospaceError = e.into();
        assert!(matches!(err, NospaceError::WsParse(_)));
    }

    #[test]
    fn test_nospace_error_from_ws_runtime_error() {
        let e = WsRuntimeError::StackUnderflow;
        let err: NospaceError = e.into();
        assert!(matches!(err, NospaceError::WsRuntime(_)));
    }
}
