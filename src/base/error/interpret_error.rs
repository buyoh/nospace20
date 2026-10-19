//! インタプリタエラー型
//!
//! nospace インタプリタ実行時に発生するエラーを表す。

use std::fmt;

/// インタプリタ実行時のエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpretError {
    /// 指定された関数が見つからない
    FunctionNotFound(String),
    /// 初期化中に予期しない制御フローが発生
    UnexpectedFlow(String),
    /// 0 による除算・剰余
    DivisionByZero,
    /// `__assert` / `__assert_not` の失敗（`__assert_not` の場合は評価値を持つ）
    AssertionFailed(Option<i64>),
    /// 未割当アドレスへのアクセス
    InvalidMemoryAccess(i64),
    /// 解放済みメモリへのアクセス
    FreedMemoryAccess(i64),
    /// 未割当アドレスの解放
    InvalidFree(i64),
    /// ブロック先頭以外のアドレスの解放
    FreeNotBlockStart {
        /// 解放しようとしたアドレス
        addr: i64,
        /// そのアドレスを含むブロックの先頭（ユーザーポインタ）
        block_start: i64,
    },
    /// 二重解放
    DoubleFree(i64),
    /// `__alloc` のサイズが負または処理系の上限を超える
    InvalidAllocSize(i64),
    /// `__putc` に出力できない文字コード
    InvalidCharCode(i64),
    /// 式評価回数の上限（`max_expression_count`）を超過
    ExpressionLimitExceeded(usize),
    /// NospaceVM のフレーム数上限（`max_vm_frames`）を超過
    StackOverflow(usize),
    /// 入出力エラー
    Io(String),
    /// 代入できない左辺への代入（意味解析で検出されない形。例: `x + 1 = 1`、構造体フィールドへの代入）
    NotAssignable,
}

impl fmt::Display for InterpretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpretError::FunctionNotFound(name) => {
                write!(f, "function '{}' not found", name)
            }
            InterpretError::UnexpectedFlow(detail) => {
                write!(f, "unexpected flow: {}", detail)
            }
            InterpretError::DivisionByZero => write!(f, "division by zero"),
            InterpretError::AssertionFailed(None) => write!(f, "assertion failed"),
            InterpretError::AssertionFailed(Some(v)) => {
                write!(f, "assertion failed: {} != 0", v)
            }
            InterpretError::InvalidMemoryAccess(addr) => {
                write!(f, "invalid memory access at address {}", addr)
            }
            InterpretError::FreedMemoryAccess(addr) => {
                write!(f, "access to freed memory at address {}", addr)
            }
            InterpretError::InvalidFree(addr) => write!(f, "free: invalid address {}", addr),
            InterpretError::FreeNotBlockStart { addr, block_start } => write!(
                f,
                "free: address {} is not a block start (block starts at {})",
                addr, block_start
            ),
            InterpretError::DoubleFree(addr) => write!(f, "double free at address {}", addr),
            InterpretError::InvalidAllocSize(size) => write!(f, "alloc: invalid size {}", size),
            InterpretError::InvalidCharCode(code) => {
                write!(f, "__putc: invalid character code {}", code)
            }
            InterpretError::ExpressionLimitExceeded(limit) => {
                write!(f, "expression evaluation limit exceeded: {}", limit)
            }
            InterpretError::StackOverflow(limit) => {
                write!(f, "stack overflow: frame limit {} exceeded", limit)
            }
            InterpretError::Io(msg) => write!(f, "I/O error: {}", msg),
            InterpretError::NotAssignable => write!(f, "left value is not assignable"),
        }
    }
}

impl std::error::Error for InterpretError {}

#[cfg(test)]
#[path = "interpret_error_tests.rs"]
mod tests;
