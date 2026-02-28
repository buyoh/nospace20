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
mod tests {
    use super::*;
    use crate::compiler_ws::alloc_runtime::test_helpers::{run_alloc_free_sequence, AllocOp};

    #[test]
    fn test_bump_memory_init_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_memory_init(5);
        assert!(!prog.is_empty(), "memory init should produce instructions");
    }

    #[test]
    fn test_bump_subroutines_has_alloc_and_free() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_subroutines();
        assert!(
            !prog.is_empty(),
            "bump allocator should have __rt_alloc/__rt_free subroutines"
        );
        // サブルーチンには Label, Return が含まれるはず
        let insts = prog.instructions();
        let has_rt_alloc_label = insts
            .iter()
            .any(|i| matches!(i, Instruction::Label(label) if *label == reserved_labels::RT_ALLOC));
        let has_rt_free_label = insts
            .iter()
            .any(|i| matches!(i, Instruction::Label(label) if *label == reserved_labels::RT_FREE));
        assert!(has_rt_alloc_label, "should contain __rt_alloc label");
        assert!(has_rt_free_label, "should contain __rt_free label");
    }

    #[test]
    fn test_bump_prologue_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_prologue(3, &[0, 1, 2]);
        assert!(!prog.is_empty(), "prologue should produce instructions");
    }

    #[test]
    fn test_bump_prologue_calls_rt_alloc() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_prologue(5, &[0, 1]);
        let insts = prog.instructions();
        let has_alloc_call = insts
            .iter()
            .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_ALLOC));
        assert!(has_alloc_call, "prologue should call __rt_alloc");
    }

    #[test]
    fn test_bump_epilogue_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_epilogue();
        assert!(!prog.is_empty(), "epilogue should produce instructions");
    }

    #[test]
    fn test_bump_epilogue_calls_rt_free() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_epilogue();
        let insts = prog.instructions();
        let has_free_call = insts
            .iter()
            .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_FREE));
        assert!(has_free_call, "epilogue should call __rt_free");
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

    /// VM 上で __rt_alloc → __rt_free → __rt_alloc を実行し、バンプ方式の動作を検証する。
    /// alloc → free → alloc の LIFO パターンで同じアドレスが返ることを確認。
    #[test]
    fn test_bump_alloc_free_on_vm() {
        use crate::whitespace::{StepResult, WhitespaceVM};

        let bump = BumpAllocRuntime;
        let mut prog = WsProgram::new();

        // メモリ初期化 (global_heap_size = 0)
        prog.append(bump.generate_memory_init(0));

        // __rt_alloc(3): ptr1 を確保
        prog.extend([
            Instruction::Push(WsNumber(3)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            // ptr1 がスタックトップ: 期待値は GLOBAL_PTR (= 8)
            Instruction::OutputNumber, // print ptr1
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar, // '\n'
        ]);

        // __rt_alloc(2): ptr2 を確保
        prog.extend([
            Instruction::Push(WsNumber(2)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            // ptr2 がスタックトップ: 期待値は 8 + 3 = 11
            Instruction::OutputNumber, // print ptr2
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar, // '\n'
        ]);

        // __rt_free(ptr2): ptr2 を解放 (LHE = 11)
        prog.extend([
            Instruction::Push(WsNumber(11)),
            Instruction::Call(reserved_labels::RT_FREE),
        ]);

        // __rt_alloc(2): ptr3 を確保 (同じアドレス 11 が返るはず)
        prog.extend([
            Instruction::Push(WsNumber(2)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            Instruction::OutputNumber, // print ptr3
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar, // '\n'
        ]);

        prog.push(Instruction::Exit);
        prog.append(bump.generate_subroutines());

        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap();
        let result = vm.run(10000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        let output = vm.get_stdout_string();
        assert_eq!(output, "8\n11\n11\n");
    }

    /// プロローグ → エピローグの完全フローをVM上で検証する。
    /// 関数呼び出し相当のフレーム確保→引数書き込み→読み出し→解放をシミュレート。
    #[test]
    fn test_bump_prologue_epilogue_on_vm() {
        use crate::whitespace::{StepResult, WhitespaceVM};

        let bump = BumpAllocRuntime;
        let mut prog = WsProgram::new();

        // メモリ初期化 (global_heap_size = 2)
        // LHB = GLOBAL_PTR(8), LHE = GLOBAL_PTR + 2 = 10
        prog.append(bump.generate_memory_init(2));

        // 呼び出し規約: 引数は順序通りに push（arg(0)が最も深い位置）
        prog.extend([
            Instruction::Push(WsNumber(42)), // arg(0) - 先に push（深い）
            Instruction::Push(WsNumber(99)), // arg(1) - 後に push（トップ）
        ]);

        // プロローグ: local_heap_size=4, arg_offsets=[0, 1]
        prog.append(bump.generate_function_prologue(4, &[0, 1]));
        // スタック: [old_LHB(=8)]
        // LHB = 10 (新フレーム先頭)
        // heap[10] = 42 (arg(0)), heap[11] = 99 (arg(1))

        // 引数を読み取って出力
        prog.extend([
            // print heap[LHB + 0] = 42
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve, // LHB = 10
            Instruction::Push(WsNumber(0)),
            Instruction::Add,
            Instruction::Retrieve, // heap[10] = 42
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
            // print heap[LHB + 1] = 99
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve, // LHB = 10
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            Instruction::Retrieve, // heap[11] = 99
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        // エピローグ: old_LHB はスタックにある
        prog.append(bump.generate_function_epilogue());

        // LHB が元の値 (8) に復元されたことを確認
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            Instruction::OutputNumber, // print LHB (should be 8)
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        // LHE がフレーム先頭 (10) に戻ったことを確認
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Retrieve,
            Instruction::OutputNumber, // print LHE (should be 10)
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        prog.push(Instruction::Exit);
        prog.append(bump.generate_subroutines());

        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap();
        let result = vm.run(10000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        let output = vm.get_stdout_string();
        assert_eq!(output, "42\n99\n8\n10\n");
    }

    // ===== BumpAllocRuntime reuse efficiency tests =====

    /// LIFO 順で free→alloc したとき、LOCAL_HEAP_END が成長しないことを検証。
    #[test]
    fn test_bump_reuse_lifo_heap_stable() {
        let bump = BumpAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 3, slot: 0 },
            AllocOp::Alloc { size: 2, slot: 1 },
            AllocOp::Free { slot: 1 },
            AllocOp::Alloc { size: 2, slot: 2 },
        ];
        let vm = run_alloc_free_sequence(&bump, 0, &ops);
        let heap = vm.heap();

        // alloc(3): ptr=8, LHE=11
        // alloc(2): ptr=11, LHE=13 (peak)
        // free(11): LHE=11
        // alloc(2): ptr=11, LHE=13 (no growth beyond peak)
        let lhe = heap.get(&heap_layout::LOCAL_HEAP_END).copied().unwrap_or(0);
        assert_eq!(
            lhe,
            heap_layout::GLOBAL_PTR + 3 + 2,
            "LOCAL_HEAP_END should not grow beyond peak after LIFO free+alloc"
        );
    }

    /// ループでの alloc/free（LIFO 順）で LOCAL_HEAP_END が一定に保たれることを検証。
    #[test]
    fn test_bump_reuse_loop_heap_stable() {
        let bump = BumpAllocRuntime;
        let mut ops = vec![AllocOp::Alloc { size: 5, slot: 0 }];
        for _ in 0..10 {
            ops.push(AllocOp::Alloc { size: 3, slot: 1 });
            ops.push(AllocOp::Free { slot: 1 });
        }
        let vm = run_alloc_free_sequence(&bump, 0, &ops);
        let heap = vm.heap();

        // alloc(5): LHE = 8+5 = 13
        // Each loop: alloc(3) → LHE=16, free → LHE=13
        // After all loops LHE should be 13 (same as after alloc(A))
        let lhe = heap.get(&heap_layout::LOCAL_HEAP_END).copied().unwrap_or(0);
        assert_eq!(
            lhe,
            heap_layout::GLOBAL_PTR + 5,
            "LOCAL_HEAP_END should remain stable after LIFO alloc/free loop"
        );
    }
}
