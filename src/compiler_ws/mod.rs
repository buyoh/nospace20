//! # Whitespace Compiler
//!
//! nospace ソースコードを Whitespace にコンパイルする。
//!
//! ## モジュール構成
//!
//! - `types` - 基本型 (WsNumber, LabelId, HeapAddress)
//! - `instruction` - 命令定義 (Instruction enum)
//! - `encoder` - バイナリエンコーダ
//! - `program` - プログラム構造 (WsProgram)
//! - `memory` - メモリレイアウト管理
//! - `label` - ラベル管理
//! - `builtin` - 組み込みルーチン生成
//! - `context` - コード生成コンテキスト
//! - `expression` - 式のコード生成
//! - `statement` - 文のコード生成

mod builtin;
mod context;
mod encoder;
mod expression;
pub mod instruction;
mod label;
mod memory;
pub mod program;
mod statement;
pub mod types;

pub use program::WsProgram;
pub use types::{HeapAddress, LabelId, WsNumber};

use crate::semantic_analyzer::Scope;
use context::CodeGenContext;

/// コンパイルエラー
#[derive(Debug)]
pub enum CompileError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            CompileError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            CompileError::MainNotFound => write!(f, "main function not found"),
            CompileError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
        }
    }
}

impl std::error::Error for CompileError {}

/// Scope を Whitespace プログラムにコンパイル
pub fn compile(scope: &Scope) -> Result<WsProgram, CompileError> {
    let mut ctx = CodeGenContext::new(scope);
    let mut program = WsProgram::new();

    // 1. ヘッダー（初期化・組み込みルーチン）を生成
    program.append(builtin::generate_header(&ctx)?);

    // 2. グローバルスコープのコードを生成
    program.append(statement::generate_scope(&mut ctx, scope)?);

    // 3. フッター（main呼び出し・終了）を生成
    program.append(builtin::generate_footer(&ctx)?);

    Ok(program)
}
