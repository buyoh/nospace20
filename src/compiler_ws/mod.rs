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
pub mod label;
pub mod memory;
pub mod program;
mod statement;
pub mod types;
pub mod alloc_runtime;

pub use program::WsProgram;

use crate::semantic_analyzer::Scope;
use context::CodeGenContext;

/// コンパイルエラー
#[derive(Debug)]
pub enum CompileError {
    #[allow(dead_code)]
    UndefinedVariable(String),
    #[allow(dead_code)]
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

/// Scope を Whitespace プログラムにコンパイル（拡張オプション付き）
pub fn compile_with_options(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
) -> Result<WsProgram, CompileError> {
    // alloc_ext に基づいてアロケータを選択
    let bump_runtime = alloc_runtime::BumpAllocRuntime;
    let fsba_runtime = alloc_runtime::FsbaFirstFitAllocRuntime;
    let alloc_runtime: &dyn alloc_runtime::AllocRuntime = if alloc_ext {
        &fsba_runtime
    } else {
        &bump_runtime
    };
    let mut ctx = CodeGenContext::new_with_options(scope, debug_ext, alloc_runtime);
    let mut program = WsProgram::new();

    // 1. ヘッダー（初期化・組み込みルーチン）を生成
    program.append(builtin::generate_header(&ctx)?);

    // 2. グローバルスコープのコードを生成
    program.append(statement::generate_scope(&mut ctx, scope)?);

    // 3. フッター（main呼び出し・終了）を生成
    program.append(builtin::generate_footer(&ctx)?);

    Ok(program)
}

