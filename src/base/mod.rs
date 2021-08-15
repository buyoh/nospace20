#[derive(Clone)] // TODO: REMOVE
pub struct CodeParseError {
    pub code_pointer: usize,
    pub message: String,
    pub internal_line: u32,
    pub internal_file: &'static str,
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
