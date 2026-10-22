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
//! - `memory` - Whitespace ヒープの予約アドレス（`heap_layout`）
//! - `label` - ラベル管理
//! - `builtin` - 組み込みルーチン生成
//! - `block_var_count` - ブロック変数の数え上げ
//! - `global_layout` - グローバル領域の配置
//! - `context` - コード生成コンテキスト
//! - `expression` - 式のコード生成
//! - `statement` - 文のコード生成

pub mod alloc_runtime;
mod block_var_count;
mod builtin;
mod context;
mod encoder;
mod expression;
mod global_layout;
pub mod instruction;
pub mod label;
pub mod memory;
mod peephole;
pub mod program;
mod statement;
pub mod types;

pub use program::WsProgram;

// base::ws_types の型を compiler_ws 直下で re-export（後方互換）
pub use crate::base::ws_types::{HeapAddress, Instruction, LabelId, WsChar, WsNumber};
use crate::semantic_analyzer::Program;
use context::CodeGenContext;
use global_layout::GlobalLayout;

// CompileError, CompileErrorKind を base::error から re-export（後方互換）
pub use crate::base::error::compile_error::{CompileError, CompileErrorKind};

/// 意味解析の結果（Program）を Whitespace プログラムにコンパイル（拡張オプション付き）
pub fn compile_with_options(
    program: &Program,
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
    let layout = GlobalLayout::compute(program, alloc_runtime.reserved_tail_slots())?;
    let mut ctx =
        CodeGenContext::new_with_options(program, &layout, debug_ext, alloc_ext, alloc_runtime);
    let mut ws_program = WsProgram::new();

    // 1. ヘッダー（初期化・組み込みルーチン）を生成
    ws_program.append(builtin::generate_header(&ctx)?);

    // 2. ルートの文・関数のコードを生成
    ws_program.append(statement::generate_program(&mut ctx, program)?);

    // 3. フッター（main呼び出し・終了）を生成
    ws_program.append(builtin::generate_footer(&ctx)?);

    Ok(ws_program)
}

/// 意味解析の結果（Program）を Whitespace プログラムにコンパイル（最適化オプション付き）
///
/// `compile_with_options` に加えて、ピープホール最適化などの
/// WsProgram レベルの最適化を制御できる。
pub fn compile_with_full_options(
    program: &Program,
    debug_ext: bool,
    alloc_ext: bool,
    apply_peephole: bool,
) -> Result<WsProgram, CompileError> {
    let ws_program = compile_with_options(program, debug_ext, alloc_ext)?;
    if apply_peephole {
        Ok(peephole::optimize(ws_program))
    } else {
        Ok(ws_program)
    }
}
