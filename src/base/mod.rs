#[derive(Clone, Debug)] // TODO: REMOVE Clone
pub struct CodeParseErrorInternal {
    // TODO: rename to CodeParseErrorInternal
    pub code_pointer: usize,
    pub message: String, // TODO: consider Cow<'static, str>
    pub internal_line: u32,
    pub internal_file: &'static str,
}

pub struct CodeParseError {
    pub code_pointer: usize,
    pub message: String, // TODO: consider Cow<'static, str>
}

#[macro_export]
macro_rules! code_parse_error {
    ($ptr: expr, $msg: expr) => {
        CodeParseErrorInternal {
            code_pointer: $ptr,
            message: $msg,
            internal_line: line!(),  // TODO: add_parse_error 内で使うとline!は意味を成さなくなる
            internal_file: file!(),
        }
    };
}

impl CodeParseErrorInternal {
    pub fn shrink(&self) -> CodeParseError {
        CodeParseError {
            code_pointer: self.code_pointer,
            message: self.message.clone(),
        }
    }
}
