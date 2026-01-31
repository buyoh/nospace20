#[derive(Clone, Debug)] // TODO: REMOVE Clone
pub struct CodeParseErrorInternal {
    pub code_pointer: Option<usize>,
    pub message: String, // TODO: consider Cow<'static, str>
    /// エラーが生成されたRustソースコードの行番号 (デバッグ用)
    ///
    /// NOTE: 現在 `code_parse_error!` マクロが `add_parse_error` 等のヘルパー関数内で
    /// 呼び出されるため、`line!()` は実際のエラー発生箇所ではなくヘルパー関数の行を指す。
    /// 正しく機能させるには、呼び出し元で `line!()` を評価してヘルパー関数に渡す必要がある。
    /// 実装コストが高いため、この機能を削除することも検討。
    pub internal_line: u32,
    /// エラーが生成されたRustソースファイル (デバッグ用)
    pub internal_file: &'static str,
}

pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: String, // TODO: consider Cow<'static, str>
}

#[macro_export]
macro_rules! code_parse_error {
    ($ptr: expr, $msg: expr) => {
        CodeParseErrorInternal {
            code_pointer: Some($ptr),
            message: $msg,
            internal_line: line!(),  // TODO: add_parse_error 内で使うとline!は意味を成さなくなる
            internal_file: file!(),
        }
    };
    ($msg: expr) => {
        CodeParseErrorInternal {
            code_pointer: None,
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
