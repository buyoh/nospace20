//! プログラムのヘッダー（メモリ初期化）・フッター（`__main` 呼び出し・ランタイムサブルーチン）生成

use crate::compiler_ws::context::CodeGenContext;
use crate::compiler_ws::{
    instruction::Instruction, program::WsProgram, CompileError, CompileErrorKind,
};

/// ヘッダー部分を生成
///
/// メモリ初期化だけを出力し、その直後からユーザーコードが続く。
/// 以前は比較用の組み込みルーチン（`COMPARATOR_*`）をここに置き、`Jump` で飛び越えていたが、
/// 比較演算・`&&` / `||`・`!` がすべてインライン分岐になったため削除した。
/// ヘッダーに組み込みルーチンを追加するときは、ユーザーコードへの `Jump` とラベルも戻すこと。
pub fn generate_header(ctx: &CodeGenContext) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // === メモリ初期化（AllocRuntime 経由） ===
    prog.append(
        ctx.alloc_runtime()
            .generate_memory_init(ctx.global_heap_size()),
    );

    Ok(prog)
}

/// フッター部分を生成
pub fn generate_footer(ctx: &CodeGenContext) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // __main 関数呼び出し
    // main_function_index を使用してラベルを取得（関数名ではなくインデックスで管理）
    let main_idx = ctx
        .program()
        .main_function_index
        .ok_or_else(|| CompileError::new(CompileErrorKind::MainNotFound))?;
    let main_label = ctx
        .get_function_label(main_idx)
        .ok_or_else(|| CompileError::new(CompileErrorKind::MainNotFound))?;
    prog.push(Instruction::Call(main_label));

    // プログラム終了
    prog.push(Instruction::Exit);

    // アロケータサブルーチン定義
    prog.append(ctx.alloc_runtime().generate_subroutines());

    Ok(prog)
}
