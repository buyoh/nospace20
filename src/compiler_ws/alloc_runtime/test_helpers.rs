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
    use crate::compiler_ws::instruction::Instruction;
    use crate::compiler_ws::label::reserved_labels;
    use crate::compiler_ws::program::WsProgram;
    use crate::compiler_ws::types::WsNumber;
    use crate::whitespace::{StepResult, WhitespaceVM};

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

    let mut vm = WhitespaceVM::from_instructions(prog.into_instructions()).unwrap();
    let result = vm.run(1_000_000);
    assert!(
        matches!(result, StepResult::Complete),
        "VM should exit normally, got: {:?}",
        result
    );
    vm
}
