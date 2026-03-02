//! パースエラー型
//!
//! トークン化・構文解析・意味解析で発生するエラーを表す。

use std::borrow::Cow;
use std::fmt;

/// ソースコードのパースエラー
///
/// 位置情報（文字インデックス）とメッセージを持つ。
#[derive(Clone, Debug)]
pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: Cow<'static, str>,
    /// デバッグビルド時のみ、エラーが発生したソースコードの位置を記録
    #[cfg(debug_assertions)]
    pub caller: &'static std::panic::Location<'static>,
}

impl CodeParseError {
    /// エラーを生成します。デバッグビルド時は呼び出し元の位置情報を自動的に記録します。
    #[track_caller]
    pub fn new(code_pointer: Option<usize>, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code_pointer,
            message: message.into(),
            #[cfg(debug_assertions)]
            caller: std::panic::Location::caller(),
        }
    }
}

impl fmt::Display for CodeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(pos) = self.code_pointer {
            write!(f, "at position {}: {}", pos, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for CodeParseError {}

#[macro_export]
macro_rules! code_parse_error {
    ($ptr: expr, $msg: expr) => {
        CodeParseError::new(Some($ptr), $msg)
    };
    ($msg: expr) => {
        CodeParseError::new(None, $msg)
    };
}

#[cfg(test)]
#[path = "parse_error_tests.rs"]
mod tests;
