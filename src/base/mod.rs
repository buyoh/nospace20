use std::borrow::Cow;

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
