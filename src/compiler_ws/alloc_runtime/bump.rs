//! バンプアロケータ（BumpAllocRuntime）の実装

use crate::compiler_ws::{
    instruction::Instruction, label::reserved_labels, memory::heap_layout, program::WsProgram,
    types::WsNumber,
};

use super::{AllocRuntime, generate_common_epilogue, generate_common_prologue};

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
        let mut prog = WsProgram::new();

        // __rt_alloc(size) → ptr
        // スタック入力: [size]
        // スタック出力: [ptr]
        // ptr = heap[LOCAL_HEAP_END]; heap[LOCAL_HEAP_END] = ptr + size; return ptr
        prog.extend([
            Instruction::Label(reserved_labels::RT_ALLOC),
            // スタック: [size]
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Retrieve,
            // スタック: [size, LHE_val]
            Instruction::Swap,
            // スタック: [LHE_val, size]
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            // スタック: [LHE_val, size, &LHE]
            Instruction::Copy(WsNumber(2)),
            // スタック: [LHE_val, size, &LHE, LHE_val]
            Instruction::Copy(WsNumber(2)),
            // スタック: [LHE_val, size, &LHE, LHE_val, size]
            Instruction::Add,
            // スタック: [LHE_val, size, &LHE, LHE_val+size]
            Instruction::Store,
            // スタック: [LHE_val, size]  heap[LHE] = LHE_val + size
            Instruction::Discard,
            // スタック: [LHE_val]  ← ptr
            Instruction::Return,
        ]);

        // __rt_free(ptr)
        // スタック入力: [ptr]
        // スタック出力: []
        // heap[LOCAL_HEAP_END] = ptr (LIFO バンプ方式)
        prog.extend([
            Instruction::Label(reserved_labels::RT_FREE),
            // スタック: [ptr]
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            // スタック: [ptr, &LHE]
            Instruction::Swap,
            // スタック: [&LHE, ptr]
            Instruction::Store,
            // heap[LHE] = ptr
            Instruction::Return,
        ]);

        prog
    }

    fn generate_function_prologue(&self, local_heap_size: i64, arg_offsets: &[i64]) -> WsProgram {
        generate_common_prologue(local_heap_size, arg_offsets)
    }

    fn generate_function_epilogue(&self) -> WsProgram {
        generate_common_epilogue()
    }
}

#[cfg(test)]
#[path = "bump_tests.rs"]
mod tests;
