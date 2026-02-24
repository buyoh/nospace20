//! # Whitespace モジュール
//!
//! Whitespace 言語のパーサとインタプリタを提供する。
//! 明示的スタックマシンとして実装されており、中断・再開が可能。

mod interpreter;
mod parser;

// compiler_ws から命令型を re-export
pub use crate::compiler_ws::instruction::Instruction;
pub use crate::compiler_ws::types::{LabelId, WsChar, WsNumber};

// パーサ
pub use parser::{parse, ParseError};

// インタプリタ
pub use interpreter::{InputWaitType, RuntimeError, StepResult, WhitespaceVM};
