//! 組み込みルーチン生成

use crate::compiler_ws::context::CodeGenContext;
use crate::compiler_ws::{
    instruction::Instruction, label::reserved_labels, memory::heap_layout, program::WsProgram,
    types::WsNumber, CompileError,
};

/// ヘッダー部分を生成
pub fn generate_header(ctx: &CodeGenContext) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // === メモリ初期化 ===

    // heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Push(WsNumber(heap_layout::GLOBAL_PTR)),
        Instruction::Store,
    ]);

    // heap[LOCAL_HEAP_END] = GLOBAL_PTR + global_size
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
        Instruction::Push(WsNumber(heap_layout::GLOBAL_PTR + ctx.global_heap_size())),
        Instruction::Store,
    ]);

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
    let main_label = ctx
        .get_function_label("main")
        .ok_or(CompileError::MainNotFound)?;
    prog.push(Instruction::Call(main_label));

    // プログラム終了
    prog.push(Instruction::Exit);

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

/// 関数開始時のローカル変数領域確保
pub fn generate_local_allocate(local_heap_size: i64) -> WsProgram {
    let mut prog = WsProgram::new();

    // 現在の local_begin をスタックに退避
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Retrieve,
    ]);

    // local_begin := local_end
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
        Instruction::Duplicate,
        Instruction::Retrieve,
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Copy(WsNumber(1)),
        Instruction::Store,
    ]);

    // local_end := local_begin + scope_size
    prog.extend([
        Instruction::Push(WsNumber(local_heap_size)),
        Instruction::Add,
        Instruction::Store,
    ]);

    prog
}

/// 関数終了時のローカル変数領域解放
pub fn generate_local_deallocate() -> WsProgram {
    let mut prog = WsProgram::new();

    // local_end := local_begin
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Retrieve,
        Instruction::Store,
    ]);

    // local_begin := スタックから復元
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Swap,
        Instruction::Store,
    ]);

    prog
}
