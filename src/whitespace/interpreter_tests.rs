use super::*;
use crate::base::ws_types::WsNumber;

#[test]
fn test_push_and_add() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(2)),
        Instruction::Push(WsNumber(3)),
        Instruction::Add,
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[5]);
}

#[test]
fn test_suspension() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(1)),
        Instruction::Push(WsNumber(2)),
        Instruction::Push(WsNumber(3)),
        Instruction::Add,
        Instruction::Add,
        Instruction::Exit,
    ])
    .unwrap();
    // budget=2 で中断
    let result = vm.step(2);
    assert_eq!(result, StepResult::Suspended);
    assert_eq!(vm.data_stack(), &[1, 2]); // 2命令分のみ実行

    // 残りを実行
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[6]);
}

#[test]
fn test_subroutine_call() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        // 0: jump to label 1
        Instruction::Jump(LabelId(1)),
        // 1: subroutine at label 2
        Instruction::Label(LabelId(2)),
        Instruction::Push(WsNumber(42)),
        Instruction::Return,
        // 4: main code at label 1
        Instruction::Label(LabelId(1)),
        Instruction::Call(LabelId(2)),
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[42]);
}

#[test]
fn test_trace_extension() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        // __trace(7): push -10, push 7, store
        Instruction::Push(WsNumber(-10)),
        Instruction::Push(WsNumber(7)),
        Instruction::Store,
        Instruction::Exit,
    ])
    .unwrap()
    .with_debug_ext(true);
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.traced.get(&7), Some(&1));
}

#[test]
fn test_heap_store_retrieve() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Push(WsNumber(42)),  // value
        Instruction::Store,
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[42]);
}

#[test]
fn test_division_by_zero() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(10)),
        Instruction::Push(WsNumber(0)),
        Instruction::Div,
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Error(RuntimeError::DivisionByZero));
}

#[test]
fn test_stack_underflow() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Add, // スタックが空なのに Add
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Error(RuntimeError::StackUnderflow));
}

#[test]
fn test_strict_heap_uninitialized_error() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Retrieve,            // 未初期化
        Instruction::Exit,
    ])
    .unwrap()
    .with_strict_heap(true);
    let result = vm.run(100);
    assert_eq!(
        result,
        StepResult::Error(RuntimeError::UninitializedHeap(100))
    );
}

#[test]
fn test_strict_heap_initialized_ok() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Push(WsNumber(42)),  // value
        Instruction::Store,
        Instruction::Push(WsNumber(100)), // addr
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap()
    .with_strict_heap(true);
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[42]);
}

#[test]
fn test_non_strict_heap_uninitialized_returns_zero() {
    // 既存動作の確認: strict-heap 無効時は未初期化アドレスに 0 を返す
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(100)),
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[0]);
}

// ===== interactive stdin テスト =====

/// InputChar: interactive モードでバッファにデータがある場合は正常読み取り
#[test]
fn test_interactive_stdin_input_char_ok() {
    // InputChar: スタックトップのアドレスへ文字コードを格納する
    // addr=10 に 'A'(65) が格納されることを確認
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(10)), // addr
        Instruction::InputChar,
        Instruction::Push(WsNumber(10)), // addr
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap()
    .with_interactive_stdin();

    vm.provide_stdin("A");
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[65]); // 'A' = 65
}

/// InputChar: バッファが空の場合は WaitingForInput(Char) を返す
#[test]
fn test_interactive_stdin_input_char_waiting() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(10)),
        Instruction::InputChar,
        Instruction::Exit,
    ])
    .unwrap()
    .with_interactive_stdin();

    // バッファ空で実行 → WaitingForInput
    let result = vm.step(100);
    assert_eq!(result, StepResult::WaitingForInput(InputWaitType::Char));

    // データ追加後に resume → Complete
    vm.provide_stdin("B");
    let result = vm.step(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(*vm.heap().get(&10).unwrap(), 66); // 'B' = 66
}

/// InputNumber: interactive モードでバッファに改行付き数値がある場合は正常読み取り
#[test]
fn test_interactive_stdin_input_number_ok() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(20)), // addr
        Instruction::InputNumber,
        Instruction::Push(WsNumber(20)),
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap()
    .with_interactive_stdin();

    vm.provide_stdin("42\n");
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(vm.data_stack(), &[42]);
}

/// InputNumber: バッファに改行なしは WaitingForInput(Number) を返す
#[test]
fn test_interactive_stdin_input_number_waiting() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(20)),
        Instruction::InputNumber,
        Instruction::Exit,
    ])
    .unwrap()
    .with_interactive_stdin();

    // バッファ空で実行 → WaitingForInput
    let result = vm.step(100);
    assert_eq!(result, StepResult::WaitingForInput(InputWaitType::Number));

    // 改行なしでデータ追加しても WaitingForInput のまま
    vm.provide_stdin("10");
    let result = vm.step(100);
    assert_eq!(result, StepResult::WaitingForInput(InputWaitType::Number));

    // 改行追加で resume
    vm.provide_stdin("\n");
    let result = vm.step(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(*vm.heap().get(&20).unwrap(), 10);
}

/// InputChar: close_stdin 後にバッファ空 → EOF (0)
#[test]
fn test_interactive_stdin_input_char_eof_after_close() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(10)),
        Instruction::InputChar,
        Instruction::Push(WsNumber(10)),
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap()
    .with_interactive_stdin();

    vm.close_stdin();
    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(*vm.heap().get(&10).unwrap(), 0); // EOF = 0
}

/// 非 interactive モードは動作変更なし（既存動作の確認）
#[test]
fn test_non_interactive_stdin_unaffected() {
    let stdin_data = b"X".to_vec();
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(5)),
        Instruction::InputChar,
        Instruction::Push(WsNumber(5)),
        Instruction::Retrieve,
        Instruction::Exit,
    ])
    .unwrap()
    .with_io(
        Box::new(std::io::BufReader::new(std::io::Cursor::new(stdin_data))),
        Box::new(Vec::<u8>::new()),
    );

    let result = vm.run(100);
    assert_eq!(result, StepResult::Complete);
    assert_eq!(*vm.heap().get(&5).unwrap(), 88); // 'X' = 88
}
