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
#[path = "interpret_error_tests.rs"]
mod tests;
