//! テスト実行ランナー

use std::fs;

use nospace20::whitespace::{StepResult, WhitespaceVM};

use super::mini_compiler::MiniCompiler;
use super::test_spec::{AllocCheck, AllocTestSpec};

/// JSON テストファイルを読み込み、コンパイル・実行・検証する
pub fn run_alloc_test(test_path: &str) {
    let json_path = format!("resources/tests_alloc/{}.test.json", test_path);
    let content = fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", json_path, e));
    let spec: AllocTestSpec = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", json_path, e));

    let mut compiler = MiniCompiler::new(&spec.vars, spec.config.global_heap_size);
    let program = compiler.compile(&spec);
    let max_steps = spec.config.max_steps;

    let instructions = program.into_instructions();
    let mut vm = WhitespaceVM::from_instructions(instructions)
        .unwrap_or_else(|e| panic!("Failed to create VM: {:?}", e))
        .with_io(
            Box::new(std::io::BufReader::new(std::io::Cursor::new(Vec::<u8>::new()))),
            Box::new(Vec::<u8>::new()),
        );

    let result = vm.run(max_steps);
    let output = vm.get_stdout_string();

    match &spec.check {
        AllocCheck::AllocIo { stdout } => {
            assert!(
                matches!(result, StepResult::Complete),
                "VM should exit normally, got: {:?}\nstdout so far: {}",
                result, output
            );
            assert_eq!(
                output, *stdout,
                "stdout mismatch"
            );
        }
        AllocCheck::AllocRuntimeError { error: _ } => {
            // ランタイムエラーの場合: VM が Error を返すか、
            // assert_var_ne 失敗でテスト失敗マーカーが出力されるかを検証
            let is_error = matches!(result, StepResult::Error(_));
            let has_fail_marker = output.contains("AF\n");
            assert!(
                is_error || has_fail_marker,
                "Expected runtime error, got: {:?}\nstdout: {}",
                result, output
            );
        }
    }
}
