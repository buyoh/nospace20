//! ランタイムメモリアロケータのコード生成
//!
//! `AllocRuntime` trait は、Whitespace コンパイル時のメモリ管理コード生成を抽象化する。
//! 実装を差し替えることで、異なるメモリ管理方式（バンプ方式、FSBA 等）を選択可能にする。
//!
//! 全ての `AllocRuntime` 実装は `__rt_alloc(size) → ptr` / `__rt_free(ptr)` サブルーチンを
//! 提供しなければならない。スタックフレーム確保もこれらのサブルーチンを経由する。

mod bump;
mod fsba;

pub use bump::BumpAllocRuntime;
pub use fsba::FsbaFirstFitAllocRuntime;

use crate::compiler_ws::{
    instruction::Instruction, label::reserved_labels, memory::heap_layout, program::WsProgram,
    types::WsNumber,
};

/// ランタイムメモリアロケータの WS コード生成を担当する trait。
///
/// 各メソッドは Whitespace 命令列 (`WsProgram`) を返す。
/// 実装を差し替えることで、異なるメモリ管理方式を選択可能にする。
pub trait AllocRuntime {
    /// ヘッダー部分のメモリ初期化コードを生成。
    ///
    /// ヒープの予約アドレスを初期化する。
    /// `global_heap_size`: グローバル変数 + static 変数の合計サイズ
    fn generate_memory_init(&self, global_heap_size: i64) -> WsProgram;

    /// フッター部分のサブルーチン定義コードを生成。
    ///
    /// アロケータが使用するサブルーチン（`__rt_alloc`, `__rt_free` 等）を定義する。
    /// 全ての実装は最低限 `__rt_alloc` と `__rt_free` を定義しなければならない。
    fn generate_subroutines(&self) -> WsProgram;

    /// 関数プロローグ: 引数コピー + フレーム確保
    ///
    /// スタック入力: `[..., arg(n-1), ..., arg(0)]`
    /// スタック出力: `[..., old_context]`
    ///
    /// 呼び出し後:
    /// - `heap[LOCAL_HEAP_BEGIN]` = 新フレーム先頭アドレス
    /// - 引数は `heap[LOCAL_HEAP_BEGIN + arg_offsets[i]]` に格納済み
    /// - `old_context` はエピローグで使用するコンテキスト復元データ
    fn generate_function_prologue(&self, local_heap_size: i64, arg_offsets: &[i64]) -> WsProgram;

    /// 関数エピローグ: フレーム解放 + コンテキスト復元
    ///
    /// スタック入力: `[..., old_context]`
    /// スタック出力: `[...]`
    fn generate_function_epilogue(&self) -> WsProgram;
}

/// 関数プロローグの共通実装（BumpAllocRuntime / FsbaFirstFitAllocRuntime で共有）。
///
/// 両アロケータで `__rt_alloc` / `__rt_free` サブルーチン経由のフレーム管理フローは同一であるため、
/// ここで共通化する。
///
/// スタック入力: `[..., arg(n-1), ..., arg(0)]`
/// スタック出力: `[..., old_LHB]`
pub(super) fn generate_common_prologue(local_heap_size: i64, arg_offsets: &[i64]) -> WsProgram {
    let mut prog = WsProgram::new();

    // 1. 現在の LOCAL_HEAP_BEGIN をスタックに退避（old_context）
    //    引数の下に配置するため、先に退避してから引数を処理する
    //    スタック: [arg(n-1), ..., arg(0)] → [arg(n-1), ..., arg(0), old_LHB]
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Retrieve,
    ]);

    // 2. __rt_alloc(local_heap_size) を呼び出し: ptr を取得
    //    スタック: [..., old_LHB] → [..., old_LHB, ptr]
    prog.extend([
        Instruction::Push(WsNumber(local_heap_size)),
        Instruction::Call(reserved_labels::RT_ALLOC),
    ]);

    // 3. LOCAL_HEAP_BEGIN = ptr
    //    スタック: [..., old_LHB, ptr] → [..., old_LHB]
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Swap,
        // スタック: [..., old_LHB, &LHB, ptr]
        Instruction::Store,
        // heap[LHB] = ptr
    ]);

    // 4. 引数を heap[LOCAL_HEAP_BEGIN + offset] にコピー
    //    スタック: [arg(n-1), ..., arg(0), old_LHB]
    //    old_LHB は引数の下に位置しているが、スタック上では引数が先に
    //    push され、old_LHB が後から push されている。
    //    引数コピーには swap で old_LHB を一時退避して引数にアクセスする。
    //    引数は offset の逆順（スタックトップから: arg(0) の offset が最後）で処理する。
    //
    //    ただし old_LHB はスタックトップにあるため、各引数の処理時に swap が必要。
    for offset in arg_offsets.iter().rev() {
        // スタック: [..., arg_i, old_LHB]
        prog.extend([
            Instruction::Swap,
            // スタック: [..., old_LHB, arg_i]
            Instruction::Push(WsNumber(*offset)),
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            Instruction::Add,
            // スタック: [..., old_LHB, arg_i, LHB+offset]
            Instruction::Swap,
            // スタック: [..., old_LHB, LHB+offset, arg_i]
            Instruction::Store,
            // heap[LHB+offset] = arg_i
            // スタック: [..., old_LHB]
        ]);
    }

    // スタック出力: [..., old_LHB]
    prog
}

/// 関数エピローグの共通実装（BumpAllocRuntime / FsbaFirstFitAllocRuntime で共有）。
///
/// スタック入力: `[..., old_LHB]`
/// スタック出力: `[...]`
pub(super) fn generate_common_epilogue() -> WsProgram {
    let mut prog = WsProgram::new();

    // スタック入力: [old_LHB]

    // 1. ptr = LOCAL_HEAP_BEGIN
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Retrieve,
        // スタック: [old_LHB, ptr]
    ]);

    // 2. LOCAL_HEAP_BEGIN = old_LHB
    prog.extend([
        Instruction::Swap,
        // スタック: [ptr, old_LHB]
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Swap,
        // スタック: [ptr, &LHB, old_LHB]
        Instruction::Store,
        // heap[LHB] = old_LHB
        // スタック: [ptr]
    ]);

    // 3. __rt_free(ptr)
    prog.extend([
        Instruction::Call(reserved_labels::RT_FREE),
        // スタック: []
    ]);

    prog
}

#[cfg(test)]
pub(super) mod test_helpers {
    use super::*;

    /// alloc/free 操作を表す列挙型
    pub enum AllocOp {
        /// __rt_alloc(size), 結果のポインタをヒープ上の slot に保存
        Alloc { size: i64, slot: i64 },
        /// __rt_free(heap[slot])
        Free { slot: i64 },
    }

    /// slot をヒープアドレスに変換（負のアドレスを使用して衝突を回避）
    pub fn slot_addr(slot: i64) -> i64 {
        -1000 - slot
    }

    /// alloc/free 操作列を受け取り、実行後の VM を返すヘルパー
    pub fn run_alloc_free_sequence(
        runtime: &dyn AllocRuntime,
        global_heap_size: i64,
        ops: &[AllocOp],
    ) -> crate::whitespace::WhitespaceVM {
        use crate::whitespace::{StepResult, WhitespaceVM};
        use crate::compiler_ws::label::reserved_labels;
        use crate::compiler_ws::program::WsProgram;
        use crate::compiler_ws::instruction::Instruction;
        use crate::compiler_ws::types::WsNumber;

        let mut prog = WsProgram::new();
        prog.append(runtime.generate_memory_init(global_heap_size));

        for op in ops {
            match op {
                AllocOp::Alloc { size, slot } => {
                    prog.extend([
                        Instruction::Push(WsNumber(*size)),
                        Instruction::Call(reserved_labels::RT_ALLOC),
                        // stack: [ptr]
                        Instruction::Push(WsNumber(slot_addr(*slot))),
                        Instruction::Swap,
                        Instruction::Store,
                        // heap[slot_addr] = ptr
                    ]);
                }
                AllocOp::Free { slot } => {
                    prog.extend([
                        Instruction::Push(WsNumber(slot_addr(*slot))),
                        Instruction::Retrieve,
                        // stack: [ptr]
                        Instruction::Call(reserved_labels::RT_FREE),
                    ]);
                }
            }
        }

        prog.push(Instruction::Exit);
        prog.append(runtime.generate_subroutines());

        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap();
        let result = vm.run(1_000_000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        vm
    }
}
