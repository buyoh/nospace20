//! コンパイルエラー型
//!
//! nospace → Whitespace コンパイル時に発生するエラーを表す。

use crate::base::location::SourceLocation;

/// コンパイルエラーの種類
#[derive(Debug, Clone)]
pub enum CompileErrorKind {
    #[allow(dead_code)]
    UndefinedVariable(String),
    #[allow(dead_code)]
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}

impl std::fmt::Display for CompileErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileErrorKind::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            CompileErrorKind::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            CompileErrorKind::MainNotFound => write!(f, "__main function not found"),
            CompileErrorKind::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
        }
    }
}

/// コンパイルエラー（位置情報付き）
#[derive(Debug)]
pub struct CompileError {
    /// エラーの種類
    pub kind: CompileErrorKind,
    /// エラーが発生したソースコードの位置（文レベル）
    /// 文の開始位置。式レベルのエラーは文の位置で代替。
    /// MainNotFound など位置特定不能なエラーは None。
    pub location: Option<SourceLocation>,
}

impl CompileError {
    /// 位置情報なしのエラーを生成
    pub fn new(kind: CompileErrorKind) -> Self {
        Self {
            kind,
            location: None,
        }
    }

    /// 位置情報付きのエラーを生成
    pub fn with_location(kind: CompileErrorKind, location: SourceLocation) -> Self {
        Self {
            kind,
            location: Some(location),
        }
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
#[path = "compile_error_tests.rs"]
mod tests;
