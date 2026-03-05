use super::*;

fn run_src(src: &str) -> (Option<i64>, String) {
    let mut vm = NospaceVM::from_source(src).expect("parse/analyze failed");
    match vm.run(1_000_000) {
        StepResult::Complete { return_value } => (return_value, vm.get_stdout_string()),
        StepResult::Error(e) => panic!("runtime error: {:?}", e),
        StepResult::Suspended => panic!("did not complete within budget"),
    }
}

#[test]
fn test_from_source_parse_error() {
    assert!(NospaceVM::from_source("this is not valid nospace!!!!").is_err());
}

#[test]
fn test_simple_return() {
    let (rv, _) = run_src("func: __main() { return: 42; }");
    assert_eq!(rv, Some(42));
}

#[test]
fn test_puti_output() {
    let (_, out) = run_src("func: __main() { __puti(123); }");
    assert_eq!(out, "123");
}

#[test]
fn test_arithmetic() {
    let (rv, _) = run_src("func: __main() { return: 3 + 4 * 2; }");
    assert_eq!(rv, Some(11));
}

#[test]
fn test_variable_assign() {
    let (rv, _) = run_src("func: __main() { let: x; x = 10; return: x + 5; }");
    assert_eq!(rv, Some(15));
}

#[test]
fn test_if_true() {
    let (rv, _) = run_src(
        "func: __main() { let: x; x = if: 1 { 10; } else: { 20; }; return: x; }");
    assert_eq!(rv, Some(10));
}

#[test]
fn test_if_false() {
    let (rv, _) = run_src(
        "func: __main() { let: x; x = if: 0 { 20; } else: { 10; }; return: x; }");
    assert_eq!(rv, Some(10));
}

#[test]
fn test_while_loop() {
    let (rv, _) = run_src(
        "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 5 { s = s + i; i = i + 1; }; return: s; }");
    assert_eq!(rv, Some(10));
}

#[test]
fn test_function_call() {
    let (rv, _) = run_src(
        "func: double(x) { return: x * 2; } func: __main() { return: double(21); }");
    assert_eq!(rv, Some(42));
}

#[test]
fn test_recursive_function() {
    // まず fib(2) = 1 を確認
    let (rv2, _) = run_src(r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(2); }"#);
    assert_eq!(rv2, Some(1), "fib(2) should be 1");

    let (rv, _) = run_src(r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(10); }"#);
    assert_eq!(rv, Some(55));
}

#[test]
fn test_step_suspension() {
    let src = "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 100 { s = s + i; i = i + 1; }; return: s; }";
    let mut vm = NospaceVM::from_source(src).unwrap();
    let r1 = vm.step(5);
    assert!(matches!(r1, StepResult::Suspended), "expected Suspended, got {:?}", r1);
    let r2 = vm.run(10_000_000);
    assert!(matches!(r2, StepResult::Complete { return_value: Some(4950) }),
        "expected Complete(4950), got {:?}", r2);
}

#[test]
fn test_complete_is_idempotent() {
    let mut vm = NospaceVM::from_source("func: __main() { return: 1; }").unwrap();
    let r1 = vm.run(1_000_000);
    assert!(matches!(r1, StepResult::Complete { return_value: Some(1) }));
    let r2 = vm.step(1);
    assert!(matches!(r2, StepResult::Complete { return_value: Some(1) }));
}

#[test]
fn test_initial_state() {
    let vm = NospaceVM::from_source("func: __main() { return: 42; }").unwrap();
    assert!(!vm.is_complete());
    assert_eq!(vm.total_steps(), 0);
    assert_eq!(vm.return_value(), None);
}

#[test]
fn test_builder_with_stdin() {
    let stdin: Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new("hello".as_bytes())));
    let vm = NospaceVM::from_source("func: __main() { return: 0; }").unwrap().with_stdin(stdin);
    assert!(!vm.is_complete());
}

#[test]
fn test_builder_with_config() {
    let vm = NospaceVM::from_source("func: __main() { return: 0; }")
        .unwrap().with_config(EnvironmentConfig::new());
    assert!(!vm.is_complete());
}

#[test]
fn test_with_io_disables_capture() {
    let stdin:  Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new(b"" as &[u8])));
    let stdout: Box<dyn Write>   = Box::new(Vec::<u8>::new());
    let vm = NospaceVM::from_source("func: __main() { return: 0; }")
        .unwrap().with_io(stdin, stdout);
    assert_eq!(vm.get_stdout_string(), "");
}

#[test]
fn test_step_result_debug() {
    let _ = format!("{:?}", StepResult::Suspended);
    let _ = format!("{:?}", StepResult::Complete { return_value: Some(1) });
    let _ = format!("{:?}", StepResult::Error(InterpretError::FunctionNotFound("f".into())));
}

#[test]
fn test_get_stdout_string_initially_empty() {
    let vm = NospaceVM::from_source("func: __main() { return: 0; }").unwrap();
    assert_eq!(vm.get_stdout_string(), "");
}

// ===== step(1) 中断・再開テスト =====

#[test]
fn test_step_one_at_a_time() {
    // step(1) を繰り返し呼んですべて実行完了できることを確認
    let src = "func: __main() { return: 1 + 2 + 3; }";
    let mut vm = NospaceVM::from_source(src).unwrap();
    let mut step_calls = 0;
    loop {
        match vm.step(1) {
            StepResult::Complete { return_value } => {
                assert_eq!(return_value, Some(6));
                break;
            }
            StepResult::Suspended => {
                step_calls += 1;
                assert!(step_calls < 1000, "step(1) loop exceeded 1000 iterations");
            }
            StepResult::Error(e) => panic!("unexpected error: {:?}", e),
        }
    }
    assert!(step_calls > 0, "should have suspended at least once");
    // total_steps は式評価回数のみカウント（GlobalInit 等はカウントしない）
    assert!(vm.total_steps() > 0, "total_steps should be > 0");
}

#[test]
fn test_step_one_with_function_call() {
    // 関数呼び出しを含む場合も step(1) で正しく実行できること
    let src = r#"
func: add(a, b) { return: a + b; }
func: __main() { return: add(10, 20); }
"#;
    let mut vm = NospaceVM::from_source(src).unwrap();
    let mut steps = 0;
    loop {
        match vm.step(1) {
            StepResult::Complete { return_value } => {
                assert_eq!(return_value, Some(30));
                break;
            }
            StepResult::Suspended => {
                steps += 1;
                assert!(steps < 10000, "step(1) loop exceeded limit");
            }
            StepResult::Error(e) => panic!("unexpected error: {:?}", e),
        }
    }
}

#[test]
fn test_step_one_with_loop() {
    // ループを含むプログラムを step(1) で実行
    let src = "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 10 { s = s + i; i = i + 1; }; return: s; }";
    let mut vm = NospaceVM::from_source(src).unwrap();
    let mut steps = 0;
    loop {
        match vm.step(1) {
            StepResult::Complete { return_value } => {
                assert_eq!(return_value, Some(45));
                break;
            }
            StepResult::Suspended => {
                steps += 1;
                assert!(steps < 100000, "step(1) loop exceeded limit");
            }
            StepResult::Error(e) => panic!("unexpected error: {:?}", e),
        }
    }
}

#[test]
fn test_step_one_with_recursion() {
    // 再帰を含むプログラムを step(1) で実行
    let src = r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(6); }
"#;
    let mut vm = NospaceVM::from_source(src).unwrap();
    let mut steps = 0;
    loop {
        match vm.step(1) {
            StepResult::Complete { return_value } => {
                assert_eq!(return_value, Some(8));
                break;
            }
            StepResult::Suspended => {
                steps += 1;
                assert!(steps < 1_000_000, "step(1) loop exceeded limit");
            }
            StepResult::Error(e) => panic!("unexpected error: {:?}", e),
        }
    }
}

#[test]
fn test_step_one_preserves_state() {
    // step(1) の合間で状態が正しく保存されることを確認
    let src = "func: __main() { __puti(1); __puti(2); __puti(3); return: 0; }";
    let mut vm = NospaceVM::from_source(src).unwrap();

    // 途中まで実行
    let mut suspended_count = 0;
    for _ in 0..3 {
        match vm.step(1) {
            StepResult::Suspended => { suspended_count += 1; }
            StepResult::Complete { .. } => break,
            StepResult::Error(e) => panic!("unexpected error: {:?}", e),
        }
    }

    // 残りを実行
    let result = vm.run(1_000_000);
    match result {
        StepResult::Complete { .. } => {}
        _ => panic!("expected completion"),
    }
    assert_eq!(vm.get_stdout_string(), "123");
}

// ===== max_expression_count 相当（Suspended + 再開） =====

#[test]
fn test_suspension_and_resume() {
    // 少ない budget で中断し、追加の budget で完了できることを確認
    let src = "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 100 { s = s + i; i = i + 1; }; return: s; }";
    let mut vm = NospaceVM::from_source(src).unwrap();

    // 少ない budget → Suspended
    let r1 = vm.step(10);
    assert!(matches!(r1, StepResult::Suspended), "expected Suspended, got {:?}", r1);
    assert!(!vm.is_complete());
    let steps_after_first = vm.total_steps();
    assert!(steps_after_first > 0);

    // 追加の budget → まだ Suspended かもしれない
    let r2 = vm.step(10);
    assert!(!matches!(r2, StepResult::Error(_)));
    let steps_after_second = vm.total_steps();
    assert!(steps_after_second > steps_after_first);

    // 十分な budget で完了
    let r3 = vm.run(1_000_000);
    assert!(matches!(r3, StepResult::Complete { return_value: Some(4950) }),
        "expected Complete(4950), got {:?}", r3);
    assert!(vm.is_complete());
}

#[test]
fn test_budget_zero_returns_suspended() {
    let mut vm = NospaceVM::from_source("func: __main() { return: 42; }").unwrap();
    let r = vm.step(0);
    assert!(matches!(r, StepResult::Suspended));
    assert!(!vm.is_complete());
}

#[test]
fn test_total_steps_increments_correctly() {
    // total_steps が式評価回数と一致することを確認
    let src = "func: __main() { return: 1 + 2; }";
    let mut vm = NospaceVM::from_source(src).unwrap();
    assert_eq!(vm.total_steps(), 0);

    vm.run(1_000_000);
    assert!(vm.total_steps() > 0, "total_steps should be > 0 after execution");
}

#[test]
fn test_repeated_suspension_accumulates_steps() {
    let src = "func: __main() { let: i; i = 0; while: i < 50 { i = i + 1; }; return: i; }";
    let mut vm = NospaceVM::from_source(src).unwrap();

    // 十分な反復で完了まで実行
    let mut suspend_count = 0;
    loop {
        match vm.step(20) {
            StepResult::Suspended => {
                suspend_count += 1;
                assert!(suspend_count < 10000, "too many suspensions");
            }
            StepResult::Complete { return_value } => {
                assert_eq!(return_value, Some(50));
                break;
            }
            StepResult::Error(e) => panic!("unexpected error: {:?}", e),
        }
    }
    assert!(suspend_count > 0, "should have suspended at least once");
    assert!(vm.total_steps() > 0, "total_steps should increase across run");
}

// ===== 再帰版インタプリタとの結果一致テスト =====

/// 再帰版インタプリタと NospaceVM の結果を比較するヘルパー
fn assert_vm_matches_interpreter(src: &str) {
    use crate::interpreter;
    use crate::interpreter::Environment;

    // 再帰版インタプリタで実行
    let tokens = crate::token_parser::parse_to_tokens(&src.to_string()).unwrap();
    let tree = crate::tree_parser::parse_to_tree(&tokens).unwrap();
    let scope = crate::semantic_analyzer::analyze(&tree).unwrap();

    let mut env = Environment::new();
    interpreter::interpret_global(&mut env, &scope).expect("global init failed");
    let interp_result = interpreter::interpret_func(&mut env, &scope, "__main");
    let interp_traced = env.traced.clone();

    // NospaceVM で実行
    let tokens2 = crate::token_parser::parse_to_tokens(&src.to_string()).unwrap();
    let tree2 = crate::tree_parser::parse_to_tree(&tokens2).unwrap();
    let scope2 = crate::semantic_analyzer::analyze(&tree2).unwrap();

    let mut vm = NospaceVM::from_scope(scope2).expect("failed to create NospaceVM");
    let vm_result = vm.run(10_000_000);

    // 結果を比較
    match (&interp_result, &vm_result) {
        (Ok(interp_rv), StepResult::Complete { return_value: vm_rv }) => {
            assert_eq!(interp_rv, vm_rv,
                "return value mismatch: interpreter={:?}, vm={:?}", interp_rv, vm_rv);
        }
        (Err(interp_err), StepResult::Error(vm_err)) => {
            // 両方エラー: OK（エラーメッセージの完全一致は不要）
            let _ = (interp_err, vm_err);
        }
        _ => {
            panic!("result type mismatch: interpreter={:?}, vm={:?}", interp_result, vm_result);
        }
    }

    // trace を比較
    assert_eq!(interp_traced, *vm.traced(),
        "trace mismatch:\n  interpreter: {:?}\n  vm: {:?}", interp_traced, vm.traced());
}

#[test]
fn test_match_simple_return() {
    assert_vm_matches_interpreter("func: __main() { return: 42; }");
}

#[test]
fn test_match_arithmetic() {
    assert_vm_matches_interpreter("func: __main() { return: (3 + 5) * 2 - 1; }");
}

#[test]
fn test_match_variables() {
    assert_vm_matches_interpreter(
        "func: __main() { let: x; let: y; x = 10; y = x * 3; return: y - x; }"
    );
}

#[test]
fn test_match_if_expression() {
    assert_vm_matches_interpreter(
        "func: __main() { let: r; r = if: 1 { 100; } else: { 200; }; return: r; }"
    );
}

#[test]
fn test_match_while_loop() {
    assert_vm_matches_interpreter(
        "func: __main() { let: i; let: s; i = 0; s = 0; while: i < 10 { s = s + i; i = i + 1; }; return: s; }"
    );
}

#[test]
fn test_match_function_call() {
    assert_vm_matches_interpreter(r#"
func: square(x) { return: x * x; }
func: __main() { return: square(7); }
"#);
}

#[test]
fn test_match_recursive_function() {
    assert_vm_matches_interpreter(r#"
func: fib(n) {
    if: n <= 1 { return: n; };
    return: fib(n - 1) + fib(n - 2);
}
func: __main() { return: fib(10); }
"#);
}

#[test]
fn test_match_trace() {
    assert_vm_matches_interpreter(r#"
func: __main() {
    __trace(0);
    __trace(0);
    __trace(1);
    return: 0;
}
"#);
}

#[test]
fn test_match_nested_scope() {
    assert_vm_matches_interpreter(r#"
func: __main() {
    let: x;
    x = 1;
    {
        let: y;
        y = 2;
        x = x + y;
    };
    return: x;
}
"#);
}

#[test]
fn test_match_for_loop() {
    assert_vm_matches_interpreter(r#"
func: __main() {
    let: s;
    s = 0;
    for: { let: i(0); } { i < 5; } { i = i + 1; } {
        s = s + i;
    };
    return: s;
}
"#);
}

#[test]
fn test_match_break_continue() {
    assert_vm_matches_interpreter(r#"
func: __main() {
    let: s;
    let: i;
    s = 0;
    i = 0;
    while: i < 20 {
        i = i + 1;
        if: i % 2 == 0 { continue:; };
        if: i > 10 { break:; };
        s = s + i;
    };
    return: s;
}
"#);
}

#[test]
fn test_match_global_variable() {
    assert_vm_matches_interpreter(r#"
let: g;
g = 100;
func: __main() {
    return: g + 1;
}
"#);
}

#[test]
fn test_match_multiple_functions() {
    assert_vm_matches_interpreter(r#"
func: add(a, b) { return: a + b; }
func: mul(a, b) { return: a * b; }
func: __main() { return: add(mul(3, 4), 5); }
"#);
}

#[test]
fn test_trace_with_io() {
    // WASM ラッパーと同じ流れ: from_scope → with_io → run
    let src = "func: __main() { let: x; x = 10; __trace(x); }";
    let tokens = crate::token_parser::parse_to_tokens(&src.to_string()).unwrap();
    let tree = crate::tree_parser::parse_to_tree(&tokens).unwrap();
    let scope = crate::semantic_analyzer::analyze(&tree).unwrap();

    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let stdout_writer: Box<dyn Write> = Box::new(SharedWriter(Rc::clone(&stdout_buf)));
    let stdin: Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new(Vec::<u8>::new())));

    let mut vm = NospaceVM::from_scope(scope).unwrap();
    vm = vm.with_io(stdin, stdout_writer);

    let result = vm.run(100000);
    assert!(matches!(result, StepResult::Complete { .. }));
    assert_eq!(vm.traced().get(&10), Some(&1), "trace should record x=10");
}
