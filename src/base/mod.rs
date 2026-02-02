#[derive(Clone, Debug)]
pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: String, // TODO: consider Cow<'static, str>
}

#[macro_export]
macro_rules! code_parse_error {
    ($ptr: expr, $msg: expr) => {
        CodeParseError {
            code_pointer: Some($ptr),
            message: $msg,
        }
    };
    ($msg: expr) => {
        CodeParseError {
            code_pointer: None,
            message: $msg,
        }
    };
}

mod location;
pub use location::SourceLocation;
