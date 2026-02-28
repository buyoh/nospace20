use std::borrow::Cow;
use std::fmt;

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

mod location;
pub use location::SourceLocation;

pub mod pure_eval;
pub mod constexpr_eval;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_parse_error_display_with_position() {
        let err = CodeParseError::new(Some(42), "unexpected token");
        assert_eq!(format!("{}", err), "at position 42: unexpected token");
    }

    #[test]
    fn test_code_parse_error_display_without_position() {
        let err = CodeParseError::new(None, "generic error");
        assert_eq!(format!("{}", err), "generic error");
    }

    #[test]
    fn test_code_parse_error_is_std_error() {
        let err = CodeParseError::new(Some(0), "test");
        let _: &dyn std::error::Error = &err;
    }
}
