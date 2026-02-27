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

pub mod alloc_runtime;
mod builtin;
mod context;
mod encoder;
mod expression;
pub mod instruction;
pub mod label;
pub mod memory;
mod peephole;
pub mod program;
mod statement;
pub mod types;

pub use program::WsProgram;

use crate::base::SourceLocation;
use crate::semantic_analyzer::Scope;
use context::CodeGenContext;

/// コンパイルエラーの種類
#[derive(Debug)]
pub enum CompileErrorKind {
    #[allow(dead_code)]
    UndefinedVariable(String),
    #[allow(dead_code)]
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}

impl std::fmt::Display for CompileErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileErrorKind::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            CompileErrorKind::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            CompileErrorKind::MainNotFound => write!(f, "main function not found"),
            CompileErrorKind::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
        }
    }
}

/// コンパイルエラー（位置情報付き）
#[derive(Debug)]
pub struct CompileError {
    /// エラーの種類
    pub kind: CompileErrorKind,
    /// エラーが発生したソースコードの位置（文レベル）
    /// Phase 1: 文の開始位置。式レベルのエラーは文の位置で代替。
    /// MainNotFound など位置特定不能なエラーは None。
    pub location: Option<SourceLocation>,
}

impl CompileError {
    /// 位置情報なしのエラーを生成
    pub fn new(kind: CompileErrorKind) -> Self {
        Self {
            kind,
            location: None,
        }
    }

    /// 位置情報付きのエラーを生成
    pub fn with_location(kind: CompileErrorKind, location: SourceLocation) -> Self {
        Self {
            kind,
            location: Some(location),
        }
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
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
    let mut ctx = CodeGenContext::new_with_options(scope, debug_ext, alloc_ext, alloc_runtime);
    let mut program = WsProgram::new();

    // 1. ヘッダー（初期化・組み込みルーチン）を生成
    program.append(builtin::generate_header(&ctx)?);

    // 2. グローバルスコープのコードを生成
    program.append(statement::generate_scope(&mut ctx, scope)?);

    // 3. フッター（main呼び出し・終了）を生成
    program.append(builtin::generate_footer(&ctx)?);

    Ok(program)
}

/// Scope を Whitespace プログラムにコンパイル（最適化オプション付き）
///
/// `compile_with_options` に加えて、ピープホール最適化などの
/// WsProgram レベルの最適化を制御できる。
pub fn compile_with_full_options(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
    apply_peephole: bool,
) -> Result<WsProgram, CompileError> {
    let program = compile_with_options(scope, debug_ext, alloc_ext)?;
    if apply_peephole {
        Ok(peephole::optimize(program))
    } else {
        Ok(program)
    }
}
