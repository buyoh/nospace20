//! Whitespace VM エラー型
//!
//! Whitespace パーサおよびインタプリタ実行時に発生するエラーを表す。

/// Whitespace VM 実行時エラー
///
/// `RuntimeError`（旧名称）から `WsRuntimeError` にリネーム。
/// 元の `whitespace::interpreter` モジュールでは `type RuntimeError = WsRuntimeError` として後方互換を維持。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsRuntimeError {
    /// スタックアンダーフロー
    StackUnderflow,
    /// ゼロ除算
    DivisionByZero,
    /// 未定義ラベルへのジャンプ
    UndefinedLabel(i64),
    /// ヒープの未初期化アドレスへのアクセス
    UninitializedHeap(i64),
    /// コールスタックアンダーフロー（ret 命令でコールスタックが空）
    CallStackUnderflow,
    /// PC が命令列の範囲外
    ProgramCounterOutOfBounds,
    /// I/O エラー
    IoError(String),
    /// アサーション失敗（拡張 API）
    AssertionFailed(i64),
}

impl std::fmt::Display for WsRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StackUnderflow => write!(f, "stack underflow"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::UndefinedLabel(id) => write!(f, "undefined label: {}", id),
            Self::UninitializedHeap(addr) => write!(f, "uninitialized heap at address {}", addr),
            Self::CallStackUnderflow => write!(f, "call stack underflow"),
            Self::ProgramCounterOutOfBounds => write!(f, "program counter out of bounds"),
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
            Self::AssertionFailed(val) => write!(f, "assertion failed: {}", val),
        }
    }
}

impl std::error::Error for WsRuntimeError {}

/// Whitespace パースエラー
///
/// `ParseError`（旧名称）から `WsParseError` にリネーム。
/// 元の `whitespace::parser` モジュールでは `type ParseError = WsParseError` として後方互換を維持。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsParseError {
    /// 不正な IMP（命令修飾パラメータ）
    InvalidImp { position: usize },
    /// 不正な命令コマンド部分
    InvalidCommand { position: usize, imp: String },
    /// 予期しないファイル終端
    UnexpectedEof { context: String },
    /// 数値リテラルのパースエラー
    InvalidNumber { position: usize },
    /// ラベルリテラルのパースエラー
    InvalidLabel { position: usize },
    /// 重複したラベル定義
    DuplicateLabel {
        label_id: i64,
        first_position: usize,
        second_position: usize,
    },
}

impl std::fmt::Display for WsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidImp { position } =>
                write!(f, "invalid IMP at position {}", position),
            Self::InvalidCommand { position, imp } =>
                write!(f, "invalid command for IMP '{}' at position {}", imp, position),
            Self::UnexpectedEof { context } =>
                write!(f, "unexpected end of file while parsing {}", context),
            Self::InvalidNumber { position } =>
                write!(f, "invalid number at position {}", position),
            Self::InvalidLabel { position } =>
                write!(f, "invalid label at position {}", position),
            Self::DuplicateLabel { label_id, first_position, second_position } =>
                write!(f, "duplicate label {} (first at {}, second at {})",
                    label_id, first_position, second_position),
        }
    }
}

impl std::error::Error for WsParseError {}

#[cfg(test)]
#[path = "ws_error_tests.rs"]
mod tests;
