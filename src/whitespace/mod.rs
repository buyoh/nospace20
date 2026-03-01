//! # Whitespace モジュール
//!
//! Whitespace 言語のパーサとインタプリタを提供する。
//! 明示的スタックマシンとして実装されており、中断・再開が可能。

mod interpreter;
mod parser;

// base::ws_types から命令型を re-export（whitespace は compiler_ws に非依存）
pub use crate::base::ws_types::{Instruction, LabelId, WsChar, WsNumber};

// パーサ
pub use parser::{parse, ParseError};

// インタプリタ
pub use interpreter::{
    HeapProfileStats, InputWaitType, InstructionCounts, ProfileStats, RuntimeError,
    StackProfileStats, StepResult, WhitespaceVM,
};
