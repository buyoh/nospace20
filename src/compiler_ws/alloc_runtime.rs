//! ランタイムメモリアロケータのコード生成
//!
//! `AllocRuntime` trait は、Whitespace コンパイル時のメモリ管理コード生成を抽象化する。
//! 実装を差し替えることで、異なるメモリ管理方式（バンプ方式、FSBA 等）を選択可能にする。

use crate::compiler_ws::{
    instruction::Instruction, memory::heap_layout, program::WsProgram, types::WsNumber,
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
    /// サブルーチンが不要な実装では空の `WsProgram` を返す。
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
    fn generate_function_prologue(
        &self,
        local_heap_size: i64,
        arg_offsets: &[i64],
    ) -> WsProgram;

    /// 関数エピローグ: フレーム解放 + コンテキスト復元
    ///
    /// スタック入力: `[..., old_context]`
    /// スタック出力: `[...]`
    fn generate_function_epilogue(&self) -> WsProgram;
}

/// バンプアロケータ（現行方式）
///
/// `LOCAL_HEAP_BEGIN` / `LOCAL_HEAP_END` によるバンプ方式のメモリ管理。
/// `--std-ext alloc` 未指定時のデフォルト動作。
pub struct BumpAllocRuntime;

impl AllocRuntime for BumpAllocRuntime {
    fn generate_memory_init(&self, global_heap_size: i64) -> WsProgram {
        let mut prog = WsProgram::new();

        // heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Push(WsNumber(heap_layout::GLOBAL_PTR)),
            Instruction::Store,
        ]);

        // heap[LOCAL_HEAP_END] = GLOBAL_PTR + global_heap_size
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Push(WsNumber(heap_layout::GLOBAL_PTR + global_heap_size)),
            Instruction::Store,
        ]);

        prog
    }

    fn generate_subroutines(&self) -> WsProgram {
        // バンプ方式にはサブルーチンは不要
        WsProgram::new()
    }

    fn generate_function_prologue(
        &self,
        local_heap_size: i64,
        arg_offsets: &[i64],
    ) -> WsProgram {
        let mut prog = WsProgram::new();

        // 引数を LOCAL_HEAP_END + offset にコピー（allocate 前の LHE が新フレーム先頭）
        // スタック順: arg(n-1) が最深、arg(0) がトップ
        // ループは (0..n).rev() で、トップ (arg(0)) から順にポップ・ストア
        // ※ i=n-1 → トップ (arg(n-1) of original push order) → offset[n-1]
        // 実際にはスタックトップが最後に push された引数
        for offset in arg_offsets.iter().rev() {
            prog.extend([
                Instruction::Push(WsNumber(*offset)),
                Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
                Instruction::Retrieve,
                Instruction::Add,
                Instruction::Swap,
                Instruction::Store,
            ]);
        }

        // 現在の LOCAL_HEAP_BEGIN をスタックに退避（old_context として残す）
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
        ]);

        // LOCAL_HEAP_BEGIN := LOCAL_HEAP_END
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Duplicate,
            Instruction::Retrieve,
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Copy(WsNumber(1)),
            Instruction::Store,
        ]);

        // LOCAL_HEAP_END := LOCAL_HEAP_BEGIN + local_heap_size
        prog.extend([
            Instruction::Push(WsNumber(local_heap_size)),
            Instruction::Add,
            Instruction::Store,
        ]);

        prog
    }

    fn generate_function_epilogue(&self) -> WsProgram {
        let mut prog = WsProgram::new();

        // LOCAL_HEAP_END := LOCAL_HEAP_BEGIN
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            Instruction::Store,
        ]);

        // LOCAL_HEAP_BEGIN := スタックから復元 (old_context)
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Swap,
            Instruction::Store,
        ]);

        prog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_memory_init_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_memory_init(5);
        assert!(
            !prog.is_empty(),
            "memory init should produce instructions"
        );
    }

    #[test]
    fn test_bump_subroutines_is_empty() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_subroutines();
        assert!(prog.is_empty(), "bump allocator should have no subroutines");
    }

    #[test]
    fn test_bump_prologue_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_prologue(3, &[0, 1, 2]);
        assert!(
            !prog.is_empty(),
            "prologue should produce instructions"
        );
    }

    #[test]
    fn test_bump_epilogue_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_epilogue();
        assert!(
            !prog.is_empty(),
            "epilogue should produce instructions"
        );
    }

    #[test]
    fn test_bump_prologue_no_args() {
        let bump = BumpAllocRuntime;
        // 引数なしの場合もプロローグは正しく生成されること
        let prog = bump.generate_function_prologue(5, &[]);
        assert!(
            !prog.is_empty(),
            "prologue with no args should produce instructions"
        );
    }
}
