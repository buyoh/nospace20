use super::*;
use crate::compiler_ws::alloc_runtime::test_helpers::{run_alloc_free_sequence, AllocOp};

#[test]
fn test_fsba_memory_init_produces_instructions() {
    let fsba = FsbaFirstFitAllocRuntime;
    let prog = fsba.generate_memory_init(5);
    assert!(!prog.is_empty(), "memory init should produce instructions");
}

#[test]
fn test_fsba_subroutines_has_alloc_and_free() {
    let fsba = FsbaFirstFitAllocRuntime;
    let prog = fsba.generate_subroutines();
    assert!(!prog.is_empty(), "FSBA should have subroutines");
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
fn test_fsba_prologue_calls_rt_alloc() {
    let fsba = FsbaFirstFitAllocRuntime;
    let prog = fsba.generate_function_prologue(5, &[0, 1]);
    let insts = prog.instructions();
    let has_alloc_call = insts
        .iter()
        .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_ALLOC));
    assert!(has_alloc_call, "prologue should call __rt_alloc");
}

#[test]
fn test_fsba_epilogue_calls_rt_free() {
    let fsba = FsbaFirstFitAllocRuntime;
    let prog = fsba.generate_function_epilogue();
    let insts = prog.instructions();
    let has_free_call = insts
        .iter()
        .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_FREE));
    assert!(has_free_call, "epilogue should call __rt_free");
}

/// VM 上で FSBA __rt_alloc → __rt_free → __rt_alloc を実行し、
/// 同一サイズクラスでのブロック再利用を検証する。
#[test]
fn test_fsba_alloc_free_reuse_on_vm() {
    use crate::whitespace::{StepResult, WhitespaceVM};

    let fsba = FsbaFirstFitAllocRuntime;
    let mut prog = WsProgram::new();

    // メモリ初期化 (global_heap_size = 0)
    // managed_start = 8, FSBA table at 8-12, AHT = 13
    prog.append(fsba.generate_memory_init(0));

    // __rt_alloc(1): size=1 → total=2 → class 0 → bump
    // block = 13, ptr = 14
    prog.extend([
        Instruction::Push(WsNumber(1)),
        Instruction::Call(reserved_labels::RT_ALLOC),
        Instruction::OutputNumber,
        Instruction::Push(WsNumber(10)),
        Instruction::OutputChar,
    ]);

    // __rt_free(14): block=13, bs=2 → class 0 free list
    prog.extend([
        Instruction::Push(WsNumber(14)),
        Instruction::Call(reserved_labels::RT_FREE),
    ]);

    // __rt_alloc(1): size=1 → total=2 → class 0 → free list has block 13 → reuse!
    // ptr = 14 (same as before)
    prog.extend([
        Instruction::Push(WsNumber(1)),
        Instruction::Call(reserved_labels::RT_ALLOC),
        Instruction::OutputNumber,
        Instruction::Push(WsNumber(10)),
        Instruction::OutputChar,
    ]);

    prog.push(Instruction::Exit);
    prog.append(fsba.generate_subroutines());

    let mut vm = WhitespaceVM::from_instructions(prog.into_instructions()).unwrap();
    let result = vm.run(10000);
    assert!(
        matches!(result, StepResult::Complete),
        "VM should exit normally, got: {:?}",
        result
    );
    let output = vm.get_stdout_string();
    // Both allocs return ptr=14 (same block reused)
    assert_eq!(output, "14\n14\n");
}

// ===== FsbaFirstFitAllocRuntime reuse efficiency tests =====

/// class 0 (サイズ 1) の alloc→free→alloc で ALLOC_HEAP_TOP が成長しないことを検証。
#[test]
fn test_fsba_reuse_class0_heap_stable() {
    let fsba = FsbaFirstFitAllocRuntime;
    let ops = vec![
        AllocOp::Alloc { size: 1, slot: 0 },
        AllocOp::Free { slot: 0 },
        AllocOp::Alloc { size: 1, slot: 1 },
    ];
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    // managed_start=8, table at 8..12, AHT initial=13
    // alloc(1): total=2, class 0, block_size=2, bump: AHT=15
    // free → push to class 0 free list
    // alloc(1): pop from class 0 free list, AHT still 15
    let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
    assert_eq!(
        aht, 15,
        "ALLOC_HEAP_TOP should not grow after class 0 free+alloc"
    );
}

/// 各サイズクラス (0-4) について alloc→free→alloc で ALLOC_HEAP_TOP が成長しないことを検証。
#[test]
fn test_fsba_reuse_each_class_heap_stable() {
    let fsba = FsbaFirstFitAllocRuntime;
    // (user_size, block_size)
    let classes: [(i64, i64); 5] = [
        (1, 2),   // class 0
        (3, 4),   // class 1
        (7, 8),   // class 2
        (15, 16), // class 3
        (31, 32), // class 4
    ];

    for (user_size, block_size) in &classes {
        let ops = vec![
            AllocOp::Alloc {
                size: *user_size,
                slot: 0,
            },
            AllocOp::Free { slot: 0 },
            AllocOp::Alloc {
                size: *user_size,
                slot: 1,
            },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        // AHT initial=13, after first alloc: 13+block_size
        let expected_aht = 13 + block_size;
        let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
        assert_eq!(
            aht, expected_aht,
            "ALLOC_HEAP_TOP should not grow for class with block_size={block_size}"
        );
    }
}

/// 異なるリクエストサイズでも同一サイズクラスに切り上げられる場合、
/// free→alloc で再利用されることを検証。
#[test]
fn test_fsba_reuse_roundup_heap_stable() {
    let fsba = FsbaFirstFitAllocRuntime;
    // alloc(2) → total=3, class 1 (block_size=4)
    // free → push to class 1 free list
    // alloc(3) → total=4, class 1, pop from free list
    let ops = vec![
        AllocOp::Alloc { size: 2, slot: 0 },
        AllocOp::Free { slot: 0 },
        AllocOp::Alloc { size: 3, slot: 1 },
    ];
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    // AHT initial=13, after alloc(2): 13+4=17, after free+alloc(3): 17
    let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
    assert_eq!(
        aht, 17,
        "ALLOC_HEAP_TOP should not grow when round-up reuses same class"
    );
}

/// 100 回の alloc/free ループで ALLOC_HEAP_TOP が一定に保たれることを検証。
#[test]
fn test_fsba_reuse_loop_heap_stable() {
    let fsba = FsbaFirstFitAllocRuntime;
    let mut ops = Vec::new();
    for _ in 0..100 {
        ops.push(AllocOp::Alloc { size: 3, slot: 0 });
        ops.push(AllocOp::Free { slot: 0 });
    }
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    // alloc(3): total=4, class 1, block_size=4. AHT: 13→17
    // Each loop: alloc pops free list (or bumps first time), free pushes
    // After 100 loops AHT should be 17
    let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
    assert_eq!(
        aht, 17,
        "ALLOC_HEAP_TOP should remain stable after 100 alloc/free loops"
    );
}

/// free 後にフリーリストが正しく更新されていることを直接検証。
#[test]
fn test_fsba_reuse_freelist_populated() {
    let fsba = FsbaFirstFitAllocRuntime;
    let ops = vec![
        AllocOp::Alloc { size: 1, slot: 0 },
        AllocOp::Free { slot: 0 },
    ];
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    // FSBA table for class 0 is at heap[table_ptr + 0]
    let table_ptr = heap.get(&heap_layout::FSBA_TABLE_PTR).copied().unwrap_or(0);
    let class0_head = heap.get(&(table_ptr + 0)).copied().unwrap_or(0);
    assert_ne!(
        class0_head, 0,
        "Class 0 free list should be populated after free"
    );
}

/// free→alloc でフリーリストからポップされ、リストが空に戻ることを検証。
#[test]
fn test_fsba_reuse_freelist_empty_after_realloc() {
    let fsba = FsbaFirstFitAllocRuntime;
    let ops = vec![
        AllocOp::Alloc { size: 1, slot: 0 },
        AllocOp::Free { slot: 0 },
        AllocOp::Alloc { size: 1, slot: 1 },
    ];
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    let table_ptr = heap.get(&heap_layout::FSBA_TABLE_PTR).copied().unwrap_or(0);
    let class0_head = heap.get(&(table_ptr + 0)).copied().unwrap_or(0);
    assert_eq!(
        class0_head, 0,
        "Class 0 free list should be empty after realloc"
    );
}

/// 32 セル超（汎用アロケータ経由）の alloc→free→alloc で ALLOC_HEAP_TOP が成長しないことを検証。
#[test]
fn test_fsba_reuse_general_heap_stable() {
    let fsba = FsbaFirstFitAllocRuntime;
    let ops = vec![
        AllocOp::Alloc { size: 40, slot: 0 },
        AllocOp::Free { slot: 0 },
        AllocOp::Alloc { size: 40, slot: 1 },
    ];
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    // alloc(40): total=41, general bump. AHT: 13→54
    // free → general free list
    // alloc(40): first-fit from free list. AHT stays 54
    let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
    assert_eq!(
        aht, 54,
        "ALLOC_HEAP_TOP should not grow after general free+alloc"
    );
}

/// 汎用フリーリスト (>32 セル) の free 後に ALLOC_FREE_HEAD が正しく更新されていることを検証。
#[test]
fn test_fsba_reuse_general_freelist_populated() {
    let fsba = FsbaFirstFitAllocRuntime;
    let ops = vec![
        AllocOp::Alloc { size: 40, slot: 0 },
        AllocOp::Free { slot: 0 },
    ];
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    let afh = heap
        .get(&heap_layout::ALLOC_FREE_HEAD)
        .copied()
        .unwrap_or(0);
    assert_ne!(
        afh, 0,
        "ALLOC_FREE_HEAD should be non-zero after general free"
    );
}

/// 異なるサイズクラスの free が互いのフリーリストに影響しないことを検証。
#[test]
fn test_fsba_reuse_mixed_class_independent() {
    let fsba = FsbaFirstFitAllocRuntime;
    let ops = vec![
        AllocOp::Alloc { size: 1, slot: 0 }, // class 0
        AllocOp::Alloc { size: 7, slot: 1 }, // class 2
        AllocOp::Free { slot: 0 },
        AllocOp::Free { slot: 1 },
    ];
    let vm = run_alloc_free_sequence(&fsba, 0, &ops);
    let heap = vm.heap();

    let table_ptr = heap.get(&heap_layout::FSBA_TABLE_PTR).copied().unwrap_or(0);
    let class0_head = heap.get(&(table_ptr + 0)).copied().unwrap_or(0);
    let class1_head = heap.get(&(table_ptr + 1)).copied().unwrap_or(0);
    let class2_head = heap.get(&(table_ptr + 2)).copied().unwrap_or(0);

    assert_ne!(class0_head, 0, "Class 0 free list should be populated");
    assert_eq!(class1_head, 0, "Class 1 free list should remain empty");
    assert_ne!(class2_head, 0, "Class 2 free list should be populated");
}

/// FSBA プロローグ → エピローグの完全フローをVM上で検証する。
#[test]
fn test_fsba_prologue_epilogue_on_vm() {
    use crate::whitespace::{StepResult, WhitespaceVM};

    let fsba = FsbaFirstFitAllocRuntime;
    let mut prog = WsProgram::new();

    // メモリ初期化 (global_heap_size = 2)
    // managed_start = 10, FSBA table at 10-14, AHT = 15
    prog.append(fsba.generate_memory_init(2));

    // 引数を push (arg(0) が深い)
    prog.extend([
        Instruction::Push(WsNumber(42)),
        Instruction::Push(WsNumber(99)),
    ]);

    // プロローグ: local_heap_size=4, arg_offsets=[0, 1]
    prog.append(fsba.generate_function_prologue(4, &[0, 1]));

    // 引数を読み取って出力
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Retrieve,
        Instruction::Push(WsNumber(0)),
        Instruction::Add,
        Instruction::Retrieve,
        Instruction::OutputNumber,
        Instruction::Push(WsNumber(10)),
        Instruction::OutputChar,
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Retrieve,
        Instruction::Push(WsNumber(1)),
        Instruction::Add,
        Instruction::Retrieve,
        Instruction::OutputNumber,
        Instruction::Push(WsNumber(10)),
        Instruction::OutputChar,
    ]);

    // エピローグ
    prog.append(fsba.generate_function_epilogue());

    // LHB が元の値 (8=GLOBAL_PTR) に復元されたことを確認
    prog.extend([
        Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
        Instruction::Retrieve,
        Instruction::OutputNumber,
        Instruction::Push(WsNumber(10)),
        Instruction::OutputChar,
    ]);

    prog.push(Instruction::Exit);
    prog.append(fsba.generate_subroutines());

    let mut vm = WhitespaceVM::from_instructions(prog.into_instructions()).unwrap();
    let result = vm.run(10000);
    assert!(
        matches!(result, StepResult::Complete),
        "VM should exit normally, got: {:?}",
        result
    );
    let output = vm.get_stdout_string();
    // alloc(4) with FSBA: total=5 → class 2 (block_size=8) → bump from AHT=15
    // block=15, ptr=16. LHB set to 16. arg[0]=42 at heap[16], arg[1]=99 at heap[17]
    // After epilogue: LHB restored to 8 (GLOBAL_PTR)
    assert_eq!(output, "42\n99\n8\n");
}
