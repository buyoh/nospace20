//! インタプリタエラー型
//!
//! nospace インタプリタ実行時に発生するエラーを表す。

use std::fmt;

/// インタプリタ実行時のエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpretError {
    /// 指定された関数が見つからない
    FunctionNotFound(String),
    /// 初期化中に予期しない制御フローが発生
    UnexpectedFlow(String),
}

impl fmt::Display for InterpretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpretError::FunctionNotFound(name) => {
                write!(f, "function '{}' not found", name)
            }
            InterpretError::UnexpectedFlow(detail) => {
                write!(f, "unexpected flow: {}", detail)
            }
        }
    }
}

impl std::error::Error for InterpretError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpret_error_function_not_found() {
        let err = InterpretError::FunctionNotFound("foo".to_string());
        assert_eq!(format!("{}", err), "function 'foo' not found");
    }

    #[test]
    fn test_interpret_error_unexpected_flow() {
        let err = InterpretError::UnexpectedFlow("in static init".to_string());
        assert_eq!(format!("{}", err), "unexpected flow: in static init");
    }

    #[test]
    fn test_interpret_error_is_std_error() {
        let err = InterpretError::FunctionNotFound("bar".to_string());
        let _: &dyn std::error::Error = &err;
    }
}
