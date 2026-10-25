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

#[test]
fn test_from_source_distinct_labels_not_duplicate() {
    // ラベル [S,T] と [T,T] を定義して exit。数値として解釈すると両方 1 になり、
    // 従来は DuplicateLabel になっていた
    let mut vm = WhitespaceVM::from_source("\n   \t\n\n  \t\t\n\n\n\n").unwrap();
    assert_eq!(vm.run(100), StepResult::Complete);
}

#[test]
fn test_from_source_jump_to_distinct_labels() {
    // jmp [T,T] → label [S,T]: push 1, pnum, exit → label [T,T]: push 2, pnum, exit
    // 衝突していると [S,T] 側へジャンプして 1 を出力する
    let src = concat!(
        "\n \n\t\t\n", // jmp TT
        "\n   \t\n",   // label ST
        "   \t\n",     // push 1
        "\t\n \t",     // pnum
        "\n\n\n",      // exit
        "\n  \t\t\n",  // label TT
        "   \t \n",    // push 2
        "\t\n \t",     // pnum
        "\n\n\n",      // exit
    );
    let mut vm = WhitespaceVM::from_source(src).unwrap();
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.get_stdout_string(), "2");
}

// ===== 算術オーバーフロー（wrapping） =====

/// a, b を push して op を実行し、結果のスタックを返す
fn run_binary(a: i64, b: i64, op: Instruction) -> (StepResult, Vec<i64>) {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(a)),
        Instruction::Push(WsNumber(b)),
        op,
        Instruction::Exit,
    ])
    .unwrap();
    let result = vm.run(100);
    (result, vm.data_stack().to_vec())
}

#[test]
fn test_add_overflow_wraps() {
    assert_eq!(
        run_binary(i64::MAX, 1, Instruction::Add),
        (StepResult::Complete, vec![i64::MIN])
    );
}

#[test]
fn test_sub_overflow_wraps() {
    assert_eq!(
        run_binary(i64::MIN, 1, Instruction::Sub),
        (StepResult::Complete, vec![i64::MAX])
    );
}

#[test]
fn test_mul_overflow_wraps() {
    assert_eq!(
        run_binary(i64::MAX, 2, Instruction::Mul),
        (StepResult::Complete, vec![-2])
    );
}

#[test]
fn test_div_min_by_minus_one_wraps() {
    assert_eq!(
        run_binary(i64::MIN, -1, Instruction::Div),
        (StepResult::Complete, vec![i64::MIN])
    );
}

#[test]
fn test_mod_min_by_minus_one_is_zero() {
    assert_eq!(
        run_binary(i64::MIN, -1, Instruction::Mod),
        (StepResult::Complete, vec![0])
    );
}

#[test]
fn test_div_by_zero_still_error() {
    let (result, _) = run_binary(1, 0, Instruction::Div);
    assert_eq!(result, StepResult::Error(RuntimeError::DivisionByZero));
    let (result, _) = run_binary(1, 0, Instruction::Mod);
    assert_eq!(result, StepResult::Error(RuntimeError::DivisionByZero));
}

// ===== 入力命令のヒープ書き込み（heap_store 経由） =====

/// 入力命令の書き込みがプロファイラのヒープ統計に反映される
#[test]
fn test_input_writes_recorded_in_heap_profile() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(3)),
        Instruction::InputChar,
        Instruction::Push(WsNumber(9)),
        Instruction::InputNumber,
        Instruction::Exit,
    ])
    .unwrap()
    .with_stdin(Box::new(std::io::Cursor::new(b"A12\n".to_vec())))
    .with_profiling(true);
    assert_eq!(vm.run(100), StepResult::Complete);
    let heap_stats = &vm.profile_stats().heap;
    assert_eq!(heap_stats.store_count, 2);
    assert_eq!(heap_stats.store_range, Some((3, 9)));
    assert_eq!(heap_stats.unique_address_count, 2);
}

/// debug 拡張ありで -10 に InputNumber すると __trace として記録され、ヒープには書き込まれない
#[test]
fn test_input_number_to_trace_addr_with_debug_ext() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(-10)),
        Instruction::InputNumber,
        Instruction::Exit,
    ])
    .unwrap()
    .with_stdin(Box::new(std::io::Cursor::new(b"7\n".to_vec())))
    .with_debug_ext(true);
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.traced.get(&7), Some(&1));
    assert!(vm.heap().get(&-10).is_none());
}

/// debug 拡張ありで -11 に InputChar し、EOF（入力値 0）だと AssertionFailed(0)
#[test]
fn test_input_char_eof_to_assert_addr_with_debug_ext() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(-11)),
        Instruction::InputChar,
        Instruction::Exit,
    ])
    .unwrap()
    .with_debug_ext(true);
    assert_eq!(
        vm.run(100),
        StepResult::Error(RuntimeError::AssertionFailed(0))
    );
}

/// debug 拡張なしでは -10 への入力は普通のヒープ書き込み
#[test]
fn test_input_to_trace_addr_without_debug_ext() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(-10)),
        Instruction::InputNumber,
        Instruction::Exit,
    ])
    .unwrap()
    .with_stdin(Box::new(std::io::Cursor::new(b"7\n".to_vec())));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert!(vm.traced.is_empty());
    assert_eq!(vm.heap().get(&-10), Some(&7));
}

/// interactive stdin で WouldBlock から再実行しても、書き込みは 1 回だけ数えられる
#[test]
fn test_input_retry_after_would_block_records_store_once() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(10)),
        Instruction::InputChar,
        Instruction::Exit,
    ])
    .unwrap()
    .with_interactive_stdin()
    .with_profiling(true);
    assert_eq!(
        vm.step(100),
        StepResult::WaitingForInput(InputWaitType::Char)
    );
    vm.provide_stdin("B");
    assert_eq!(vm.step(100), StepResult::Complete);
    assert_eq!(vm.profile_stats().heap.store_count, 1);
}

// ===== __clog 拡張 API (-13) =====

/// `__clog(val)` を -13 への store として実行する命令列
fn clog_program(val: i64) -> Vec<Instruction> {
    vec![
        Instruction::Push(WsNumber(-13)),
        Instruction::Push(WsNumber(val)),
        Instruction::Store,
        Instruction::Exit,
    ]
}

/// debug 拡張あり・clog writer ありで、writer に `__clog: n` を書き、stdout とヒープは変わらない
#[test]
fn test_clog_writes_to_clog_writer() {
    let clog_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let mut vm = WhitespaceVM::from_instructions(clog_program(5))
        .unwrap()
        .with_debug_ext(true)
        .with_clog_writer(Box::new(crate::base::shared_writer::SharedWriter(
            Rc::clone(&clog_buf),
        )));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(String::from_utf8_lossy(&clog_buf.borrow()), "__clog: 5\n");
    assert_eq!(vm.get_stdout_string(), "");
    assert!(vm.heap().is_empty());
}

/// clog writer の前にプログラムの stdout を flush する（出力順を保つため）
#[test]
fn test_clog_flushes_stdout_before_writing() {
    /// write はバッファに溜め、flush で共有の出力ログに移す writer
    struct BufferedLogWriter {
        pending: Vec<u8>,
        log: Rc<RefCell<Vec<u8>>>,
    }
    impl Write for BufferedLogWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.pending.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.log.borrow_mut().append(&mut self.pending);
            Ok(())
        }
    }

    let log = Rc::new(RefCell::new(Vec::<u8>::new()));
    let mut instructions = vec![Instruction::Push(WsNumber(1)), Instruction::OutputNumber];
    instructions.extend(clog_program(2));
    let mut vm = WhitespaceVM::from_instructions(instructions)
        .unwrap()
        .with_stdout(Box::new(BufferedLogWriter {
            pending: Vec::new(),
            log: Rc::clone(&log),
        }))
        .with_debug_ext(true)
        .with_clog_writer(Box::new(crate::base::shared_writer::SharedWriter(
            Rc::clone(&log),
        )));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(String::from_utf8_lossy(&log.borrow()), "1__clog: 2\n");
}

/// debug 拡張あり・clog writer なしで、何も出力せず、ヒープも変わらない
#[test]
fn test_clog_without_writer_discards() {
    let mut vm = WhitespaceVM::from_instructions(clog_program(5))
        .unwrap()
        .with_debug_ext(true);
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.get_stdout_string(), "");
    assert!(vm.heap().is_empty());
}

/// debug 拡張なしでは -13 への store は普通のヒープ書き込み
#[test]
fn test_clog_addr_without_debug_ext_is_heap_write() {
    let clog_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let mut vm = WhitespaceVM::from_instructions(clog_program(5))
        .unwrap()
        .with_clog_writer(Box::new(crate::base::shared_writer::SharedWriter(
            Rc::clone(&clog_buf),
        )));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert!(clog_buf.borrow().is_empty());
    assert_eq!(vm.heap().get(&-13), Some(&5));
}

// ===== メモリ上限 =====

/// 新しいアドレス 0, 1, 2, ... に書き込み続ける命令列（Exit に到達しない）
fn heap_growing_program() -> Vec<Instruction> {
    vec![
        // [addr]
        Instruction::Push(WsNumber(0)),
        Instruction::Label(LabelId(1)),
        // heap[addr] = 0
        Instruction::Duplicate,
        Instruction::Push(WsNumber(0)),
        Instruction::Store,
        // addr += 1
        Instruction::Push(WsNumber(1)),
        Instruction::Add,
        Instruction::Jump(LabelId(1)),
    ]
}

/// ヒープを伸ばし続けるプログラムも上限で止まる
///
/// Store はオペランド 2 つを pop してからヒープを 1 セル増やすため、使用量の最大は
/// 直前の push で決まる。そのため、上限に掛かるのは値の Push（ヒープ 8 + スタック 2 + 1 > 10）である
#[test]
fn test_memory_limit_heap_growing_program() {
    let mut vm = WhitespaceVM::from_instructions(heap_growing_program())
        .unwrap()
        .with_max_memory_cells(Some(10));
    assert_eq!(
        vm.run(10_000),
        StepResult::Error(RuntimeError::MemoryLimitExceeded(10))
    );
    assert_eq!(vm.heap().len(), 8);
    assert_eq!(vm.data_stack().len(), 2);
}

/// ヒープ: 使用量が上限に達している状態での新しいアドレスへの書き込みは MemoryLimitExceeded
///
/// 現行の命令列からは到達しない（上記のとおり push が先に上限に掛かる）ため、heap_store を直接呼ぶ
#[test]
fn test_memory_limit_heap_store_new_address() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(1)),
        Instruction::Push(WsNumber(2)),
        Instruction::Exit,
    ])
    .unwrap()
    .with_max_memory_cells(Some(3));
    assert_eq!(vm.step(2), StepResult::Suspended);
    assert_eq!(vm.heap_store(0, 9), Ok(())); // 使用量 3 セルで上限ちょうど
    assert_eq!(
        vm.heap_store(1, 9),
        Err(RuntimeError::MemoryLimitExceeded(3))
    );
    assert_eq!(vm.heap_store(0, 10), Ok(())); // 既存のアドレスへの上書きは増えない
    assert_eq!(vm.heap().len(), 1);
    assert_eq!(vm.heap().get(&0), Some(&10));
}

/// ヒープ: 既存のアドレスへの上書きでは使用セル数が増えないので、上限ちょうどでも成功する
#[test]
fn test_memory_limit_heap_overwrite_existing_address() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(0)),
        Instruction::Push(WsNumber(1)),
        Instruction::Store,
        Instruction::Push(WsNumber(5)),
        Instruction::Push(WsNumber(6)),
        Instruction::Store, // heap = {0: 1, 5: 6}
        Instruction::Push(WsNumber(0)),
        Instruction::Push(WsNumber(2)), // heap 2 + stack 2 = 4 セルで上限ちょうど
        Instruction::Store,             // 既存アドレスへの上書き
        Instruction::Exit,
    ])
    .unwrap()
    .with_max_memory_cells(Some(4));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.heap().get(&0), Some(&2));
}

/// ヒープ: 使用量が上限ちょうどでも、debug 拡張のフック（ヒープに入らない）は動作する
#[test]
fn test_memory_limit_debug_hook_at_limit() {
    let clog_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(0)),
        Instruction::Push(WsNumber(1)),
        Instruction::Store, // heap 1 セル
        Instruction::Push(WsNumber(-10)),
        Instruction::Push(WsNumber(7)),
        Instruction::Store, // __trace(7): pop 後の使用量は 1 セルで上限ちょうど
        Instruction::Push(WsNumber(-13)),
        Instruction::Push(WsNumber(8)),
        Instruction::Store, // __clog(8)
        Instruction::Exit,
    ])
    .unwrap()
    .with_debug_ext(true)
    .with_clog_writer(Box::new(crate::base::shared_writer::SharedWriter(
        Rc::clone(&clog_buf),
    )))
    .with_max_memory_cells(Some(3));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.traced.get(&7), Some(&1));
    assert_eq!(String::from_utf8_lossy(&clog_buf.borrow()), "__clog: 8\n");
}

/// ヒープ: 入力命令の書き込みにも上限が掛かる（オペランドの pop 後にエラー）
#[test]
fn test_memory_limit_input_write() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(0)),
        Instruction::InputNumber,
        Instruction::Exit,
    ])
    .unwrap()
    .with_stdin(Box::new(std::io::Cursor::new(b"7\n".to_vec())))
    .with_max_memory_cells(Some(1));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.heap().get(&0), Some(&7));

    let mut vm0 = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(0)),
        Instruction::InputNumber,
        Instruction::Exit,
    ])
    .unwrap()
    .with_stdin(Box::new(std::io::Cursor::new(b"7\n".to_vec())))
    .with_max_memory_cells(Some(0));
    // 上限 0 では最初の Push で失敗する
    assert_eq!(
        vm0.run(100),
        StepResult::Error(RuntimeError::MemoryLimitExceeded(0))
    );
}

/// データスタック: Push / Duplicate / Copy の繰り返しで MemoryLimitExceeded
#[test]
fn test_memory_limit_data_stack_push() {
    for grow in [
        Instruction::Push(WsNumber(1)),
        Instruction::Duplicate,
        Instruction::Copy(WsNumber(0)),
    ] {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(1)),
            Instruction::Label(LabelId(1)),
            grow.clone(),
            Instruction::Jump(LabelId(1)),
        ])
        .unwrap()
        .with_max_memory_cells(Some(5));
        assert_eq!(
            vm.run(10_000),
            StepResult::Error(RuntimeError::MemoryLimitExceeded(5)),
            "{:?}",
            grow
        );
        assert_eq!(vm.data_stack().len(), 5, "{:?}", grow);
    }
}

/// データスタック: 正味で増えない命令（算術演算・Retrieve・Swap）は上限ちょうどでも成功する
#[test]
fn test_memory_limit_net_zero_instructions_at_limit() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(2)),
        Instruction::Push(WsNumber(3)), // stack 2 セルで上限ちょうど
        Instruction::Add,
        Instruction::Push(WsNumber(5)), // stack 2 セルで上限ちょうど
        Instruction::Retrieve,          // pop して push（heap[5] は未初期化で 0）
        Instruction::Swap,
        Instruction::Exit,
    ])
    .unwrap()
    .with_max_memory_cells(Some(2));
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.data_stack(), &[0, 5]);
}

/// 戻り先: 自分自身を Call し続けるプログラムで MemoryLimitExceeded
#[test]
fn test_memory_limit_call_stack() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Label(LabelId(1)),
        Instruction::Call(LabelId(1)),
    ])
    .unwrap()
    .with_max_memory_cells(Some(8));
    assert_eq!(
        vm.run(10_000),
        StepResult::Error(RuntimeError::MemoryLimitExceeded(8))
    );
    assert_eq!(vm.call_stack_depth(), 8);
}

/// None（既定）では同じプログラムが上限なしで進み、ステップ数の上限で Suspended になる
#[test]
fn test_memory_limit_none_is_unlimited() {
    let mut vm = WhitespaceVM::from_instructions(heap_growing_program()).unwrap();
    assert_eq!(vm.run(10_000), StepResult::Suspended);
    assert!(vm.heap().len() > 100);

    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Label(LabelId(1)),
        Instruction::Call(LabelId(1)),
    ])
    .unwrap()
    .with_max_memory_cells(None);
    assert_eq!(vm.run(10_000), StepResult::Suspended);
}

/// Call: 使用量が上限に達していても、未定義ラベルのエラーを優先する
#[test]
fn test_memory_limit_call_undefined_label_reported_first() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(1)),
        Instruction::Call(LabelId(9)),
    ])
    .unwrap()
    .with_max_memory_cells(Some(1));
    assert!(matches!(
        vm.run(100),
        StepResult::Error(RuntimeError::UndefinedLabel(_))
    ));
}

// ===== InputNumber の入力の解釈（wsc の readnum と同じ規則） =====

/// InputNumber を 2 回行い、読んだ値を heap[0], heap[1] に置くプログラム
fn read_two_numbers_program() -> Vec<Instruction> {
    vec![
        Instruction::Push(WsNumber(0)),
        Instruction::InputNumber,
        Instruction::Push(WsNumber(1)),
        Instruction::InputNumber,
        Instruction::Exit,
    ]
}

/// Buffered stdin で実行し、結果と読めた値（heap[0], heap[1]）を返す
fn run_read_two_buffered(input: &[u8]) -> (StepResult, Option<i64>, Option<i64>) {
    let mut vm = WhitespaceVM::from_instructions(read_two_numbers_program())
        .unwrap()
        .with_stdin(Box::new(std::io::Cursor::new(input.to_vec())));
    let result = vm.run(100);
    (
        result,
        vm.heap().get(&0).copied(),
        vm.heap().get(&1).copied(),
    )
}

/// Interactive stdin（入力を全部渡して close）で実行し、結果と読めた値を返す
fn run_read_two_interactive(input: &[u8]) -> (StepResult, Option<i64>, Option<i64>) {
    let mut vm = WhitespaceVM::from_instructions(read_two_numbers_program())
        .unwrap()
        .with_interactive_stdin();
    vm.provide_stdin(std::str::from_utf8(input).unwrap());
    vm.close_stdin();
    let result = vm.run(100);
    (
        result,
        vm.heap().get(&0).copied(),
        vm.heap().get(&1).copied(),
    )
}

/// Buffered / Interactive の両方で同じ結果になることを確かめる
fn assert_read_two(input: &[u8], expected: (StepResult, Option<i64>, Option<i64>)) {
    assert_eq!(
        run_read_two_buffered(input),
        expected,
        "buffered {:?}",
        input
    );
    assert_eq!(
        run_read_two_interactive(input),
        expected,
        "interactive {:?}",
        input
    );
}

#[test]
fn test_input_number_wsc_compatible_forms() {
    assert_read_two(
        b"  +5 \n1_000\n",
        (StepResult::Complete, Some(5), Some(1000)),
    );
    assert_read_two(
        b"-9223372036854775808\r\n7",
        (StepResult::Complete, Some(i64::MIN), Some(7)),
    );
}

/// NBSP（UTF-8）も前後の空白として除く。従来の Interactive は Latin-1 で読んでいたためエラーだった
#[test]
fn test_input_number_utf8_whitespace_is_trimmed_on_both_paths() {
    assert_read_two(
        "\u{a0}5\n6\u{a0}\n".as_bytes(),
        (StepResult::Complete, Some(5), Some(6)),
    );
}

/// 1 行に 2 つの数があっても 1 行 = 1 整数（wsc と同じくエラー）
#[test]
fn test_input_number_two_numbers_on_one_line_is_error() {
    assert_read_two(
        b"1 2\n3\n",
        (
            StepResult::Error(RuntimeError::InvalidNumberInput("1 2".to_string())),
            None,
            None,
        ),
    );
}

#[test]
fn test_input_number_empty_line_is_error() {
    assert_read_two(
        b"\n5\n",
        (
            StepResult::Error(RuntimeError::InvalidNumberInput(String::new())),
            None,
            None,
        ),
    );
}

#[test]
fn test_input_number_eof_is_error() {
    assert_read_two(
        b"7\n",
        (
            StepResult::Error(RuntimeError::IoError("unexpected end of input".to_string())),
            Some(7),
            None,
        ),
    );
}

#[test]
fn test_input_number_out_of_i64_range_is_error() {
    assert_read_two(
        b"9223372036854775808\n1\n",
        (
            StepResult::Error(RuntimeError::InvalidNumberInput(
                "9223372036854775808".to_string(),
            )),
            None,
            None,
        ),
    );
}

/// 不正な UTF-8 のバイトは U+FFFD になり、数字ではないのでエラー（Buffered は従来 I/O エラー）
#[test]
fn test_input_number_invalid_utf8_is_invalid_number_input() {
    assert_eq!(
        run_read_two_buffered(b"\xff5\n"),
        (
            StepResult::Error(RuntimeError::InvalidNumberInput("\u{fffd}5".to_string())),
            None,
            None,
        )
    );
}

// ===== Copy / index_from_top =====

#[test]
fn test_index_from_top_in_range() {
    assert_eq!(index_from_top(3, 0), Some(2));
    assert_eq!(index_from_top(3, 2), Some(0));
    assert_eq!(index_from_top(3, 3), None);
    assert_eq!(index_from_top(0, 0), None);
}

/// wasm32 で usize に切り詰められていた深さ（2^32）は範囲外になる
#[test]
fn test_index_from_top_does_not_truncate_depth() {
    assert_eq!(index_from_top(1, 1u64 << 32), None);
}

/// depth + 1 が u64 で溢れる場合も範囲外になる
#[test]
fn test_index_from_top_depth_add_overflow() {
    assert_eq!(index_from_top(1, u64::MAX), None);
}

/// wasm32 で `1 + depth` が溢れて trap していた深さ（u32::MAX）は範囲外になる
#[test]
fn test_index_from_top_u32_max_depth() {
    assert_eq!(index_from_top(1, u32::MAX as u64), None);
}

fn run_copy_on_stack(stack: &[i64], n: i64) -> (StepResult, Vec<i64>) {
    let mut insts: Vec<Instruction> = stack
        .iter()
        .map(|&v| Instruction::Push(WsNumber(v)))
        .collect();
    insts.push(Instruction::Copy(WsNumber(n)));
    insts.push(Instruction::Exit);
    let mut vm = WhitespaceVM::from_instructions(insts).unwrap();
    let result = vm.run(100);
    (result, vm.data_stack().to_vec())
}

#[test]
fn test_copy_out_of_range_is_stack_underflow() {
    for n in [u32::MAX as i64, 1i64 << 32, i64::MIN, i64::MAX, 3] {
        let (result, _) = run_copy_on_stack(&[1, 2, 3], n);
        assert_eq!(
            result,
            StepResult::Error(RuntimeError::StackUnderflow),
            "Copy({})",
            n
        );
    }
}

/// 負の n は |n| として扱う（本処理系の挙動。将来の独自拡張のため予約）
#[test]
fn test_copy_negative_is_absolute_value() {
    let (result_neg, stack_neg) = run_copy_on_stack(&[10, 20, 30], -1);
    let (result_pos, stack_pos) = run_copy_on_stack(&[10, 20, 30], 1);
    assert_eq!(result_neg, result_pos);
    assert_eq!(stack_neg, stack_pos);
    assert_eq!(stack_neg, vec![10, 20, 30, 20]);
}

// ===== Slide =====

fn run_slide_on_stack(stack: &[i64], n: i64) -> (StepResult, Vec<i64>) {
    let mut insts: Vec<Instruction> = stack
        .iter()
        .map(|&v| Instruction::Push(WsNumber(v)))
        .collect();
    insts.push(Instruction::Slide(WsNumber(n)));
    insts.push(Instruction::Exit);
    let mut vm = WhitespaceVM::from_instructions(insts).unwrap();
    let result = vm.run(100);
    (result, vm.data_stack().to_vec())
}

#[test]
fn test_slide_keeps_top() {
    assert_eq!(
        run_slide_on_stack(&[1, 2, 3], 1),
        (StepResult::Complete, vec![1, 3])
    );
    assert_eq!(
        run_slide_on_stack(&[1, 2, 3], 2),
        (StepResult::Complete, vec![3])
    );
    assert_eq!(
        run_slide_on_stack(&[1, 2, 3], 0),
        (StepResult::Complete, vec![1, 2, 3])
    );
}

#[test]
fn test_slide_out_of_range_is_stack_underflow() {
    for (stack, n) in [
        (&[1, 2, 3][..], 3),
        (&[][..], 0),
        (&[1, 2, 3][..], i64::MAX),
        (&[1, 2, 3][..], 1i64 << 32),
    ] {
        let (result, _) = run_slide_on_stack(stack, n);
        assert_eq!(
            result,
            StepResult::Error(RuntimeError::StackUnderflow),
            "Slide({}) on {:?}",
            n,
            stack
        );
    }
}

/// 負の n は将来の独自拡張のため予約しており、スタックが十分深くてもエラーにする（Copy と異なり |n| にしない）
#[test]
fn test_slide_negative_is_reserved_error() {
    for n in [-1, i64::MIN] {
        let (result, stack) = run_slide_on_stack(&[1, 2, 3, 4], n);
        assert_eq!(
            result,
            StepResult::Error(RuntimeError::StackUnderflow),
            "Slide({})",
            n
        );
        assert_eq!(stack, vec![1, 2, 3, 4], "Slide({})", n);
    }
}

#[test]
fn test_slide_profiling_count() {
    let mut vm = WhitespaceVM::from_instructions(vec![
        Instruction::Push(WsNumber(1)),
        Instruction::Push(WsNumber(2)),
        Instruction::Slide(WsNumber(1)),
        Instruction::Exit,
    ])
    .unwrap()
    .with_profiling(true);
    assert_eq!(vm.run(100), StepResult::Complete);
    assert_eq!(vm.profile_stats().instruction_counts.slide, 1);
}
