#[derive(Clone, Debug)] // TODO: REMOVE Clone
pub struct CodeParseError {
    // TODO: rename to CodeParseErrorInternal
    pub code_pointer: usize,
    pub message: String, // TODO: consider Cow<'static, str>
    pub internal_line: u32,
    pub internal_file: &'static str,
}

pub struct CodeParseErrorTiny {
    pub code_pointer: usize,
    pub message: String, // TODO: consider Cow<'static, str>
}

#[macro_export]
macro_rules! code_parse_error {
    ($ptr: expr, $msg: expr) => {
        CodeParseError {
            code_pointer: $ptr,
            message: $msg,
            internal_line: line!(),
            internal_file: file!(),
        }
    };
}

impl CodeParseError {
    pub fn shrink(&self) -> CodeParseErrorTiny {
        CodeParseErrorTiny {
            code_pointer: self.code_pointer,
            message: self.message.clone(),
        }
    }
}
