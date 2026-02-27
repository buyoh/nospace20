//! 組み込みルーチン生成

use crate::compiler_ws::context::CodeGenContext;
use crate::compiler_ws::{
    instruction::Instruction, label::reserved_labels, program::WsProgram, types::WsNumber,
    CompileError, CompileErrorKind,
};

/// ヘッダー部分を生成
pub fn generate_header(ctx: &CodeGenContext) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // === メモリ初期化（AllocRuntime 経由） ===
    prog.append(
        ctx.alloc_runtime()
            .generate_memory_init(ctx.global_heap_size()),
    );

    // === ユーザーコードへジャンプ ===
    prog.push(Instruction::Jump(reserved_labels::USER_CODE_BEGIN));

    // === 組み込みルーチン ===
    prog.append(generate_comparator_zero());
    prog.append(generate_comparator_negative());
    prog.append(generate_comparator_and());
    prog.append(generate_comparator_or());

    // === ユーザーコード開始ラベル ===
    prog.push(Instruction::Label(reserved_labels::USER_CODE_BEGIN));

    Ok(prog)
}

/// フッター部分を生成
pub fn generate_footer(ctx: &CodeGenContext) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // main 関数呼び出し
    // main_function_index を使用してラベルを取得（関数名ではなくインデックスで管理）
    let main_idx = ctx
        .scope()
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

/// ゼロ判定ルーチン
///
/// 入力スタック: `[..., zero_result, nonzero_result, value]`
/// 出力スタック: `[..., result]`
fn generate_comparator_zero() -> WsProgram {
    use reserved_labels::*;

    let mut prog = WsProgram::new();

    prog.extend([
        // ラベル定義
        Instruction::Label(COMPARATOR_ZERO),
        // value == 0 なら分岐
        Instruction::JumpIfZero(COMPARATOR_ZERO_2),
        // value != 0: swap して nonzero_result を上に
        Instruction::Swap,
        // 分岐先ラベル
        Instruction::Label(COMPARATOR_ZERO_2),
        // 不要な値を破棄
        Instruction::Discard,
        // 呼び出し元へ戻る
        Instruction::Return,
    ]);

    prog
}

/// 負数判定ルーチン
///
/// 入力スタック: `[..., negative_result, nonnegative_result, value]`
/// 出力スタック: `[..., result]`
fn generate_comparator_negative() -> WsProgram {
    use reserved_labels::*;

    let mut prog = WsProgram::new();

    prog.extend([
        Instruction::Label(COMPARATOR_NEGATIVE),
        Instruction::JumpIfNegative(COMPARATOR_NEGATIVE_2),
        Instruction::Swap,
        Instruction::Label(COMPARATOR_NEGATIVE_2),
        Instruction::Discard,
        Instruction::Return,
    ]);

    prog
}

/// AND ルーチン
///
/// 入力スタック: `[..., value1, value2]`
/// 出力スタック: `[..., result]` (両方が非ゼロなら 1、それ以外は 0)
fn generate_comparator_and() -> WsProgram {
    use reserved_labels::*;

    let mut prog = WsProgram::new();

    prog.extend([
        // エントリポイント
        Instruction::Label(COMPARATOR_AND),
        // value2 == 0 なら偽へジャンプ
        Instruction::JumpIfZero(COMPARATOR_AND_2),
        // ダミー値を複製（後で discard するため）
        Instruction::Duplicate,
        // value1 == 0 なら偽へジャンプ
        Instruction::JumpIfZero(COMPARATOR_AND_2),
        // 両方真
        Instruction::Discard,
        Instruction::Push(WsNumber(1)),
        Instruction::Return,
        // 偽
        Instruction::Label(COMPARATOR_AND_2),
        Instruction::Discard,
        Instruction::Push(WsNumber(0)),
        Instruction::Return,
    ]);

    prog
}

/// OR ルーチン
///
/// 入力スタック: `[..., value1, value2]`
/// 出力スタック: `[..., result]` (どちらかが非ゼロなら 1、両方ゼロなら 0)
fn generate_comparator_or() -> WsProgram {
    use reserved_labels::*;

    let mut prog = WsProgram::new();

    prog.extend([
        // エントリポイント
        Instruction::Label(COMPARATOR_OR),
        // value2 == 0 ならチェック続行
        Instruction::JumpIfZero(COMPARATOR_OR_2),
        // value2 != 0 なので真
        Instruction::Discard,
        Instruction::Push(WsNumber(1)),
        Instruction::Return,
        // value2 == 0 だったので value1 をチェック
        Instruction::Label(COMPARATOR_OR_2),
        Instruction::JumpIfZero(COMPARATOR_OR_3),
        // value1 != 0 なので真
        Instruction::Push(WsNumber(1)),
        Instruction::Return,
        // 両方偽
        Instruction::Label(COMPARATOR_OR_3),
        Instruction::Push(WsNumber(0)),
        Instruction::Return,
    ]);

    prog
}
