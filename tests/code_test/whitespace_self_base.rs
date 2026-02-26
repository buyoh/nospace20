use std::{fs, io};

use nospace20::whitespace::{StepResult, WhitespaceVM};
use nospace20::{
    compile_to_whitespace_with_options, parse_to_tokens, parse_to_tree, syntactic_analyze,
};

/// `--opt all` 相当の最適化を適用した上で Whitespace にコンパイルして実行する
pub fn test_whitespace_self_base_opt_all(test_name: &str) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let mut a = syntactic_analyze(&s).unwrap();
    nospace20::optimize(&mut a, &nospace20::OptimizationOptions::all());
    let ws_code = compile_to_whitespace_with_options(&a, false, false)
        .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );

    let mut vm = WhitespaceVM::from_source(&ws_code)
        .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e));

    let result = vm.run(1_000_000);

    match result {
        StepResult::Complete => {}
        StepResult::Suspended => panic!(
            "Whitespace execution suspended (exceeded step limit) for {}",
            test_name
        ),
        StepResult::Error(e) => panic!("Whitespace execution failed for {}: {:?}", test_name, e),
        StepResult::WaitingForInput(t) => panic!(
            "Whitespace execution unexpectedly waiting for input ({:?}) for {}",
            t, test_name
        ),
    }
}

/// `--opt all` 相当の最適化を適用した上で Whitespace にコンパイルして IO テストを実行する
pub fn test_whitespace_self_io_base_opt_all(test_name: &str) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let check_json_value: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();

    let check_json: super::test_config::TestConfig =
        serde_json::from_value(check_json_value).ok().unwrap();
    let test_cases = check_json.get_io_test_cases();

    // コンパイル・最適化（全ケース共通）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let mut a = syntactic_analyze(&s).unwrap();
    nospace20::optimize(&mut a, &nospace20::OptimizationOptions::all());
    let ws_code = compile_to_whitespace_with_options(&a, false, false)
        .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );

    for (idx, case) in test_cases.iter().enumerate() {
        let case_name = case
            .name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format!("case_{}", idx));

        let stdin_content = if let Some(s) = &case.stdin {
            s.clone()
        } else if let Some(f) = &case.stdin_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap_or_default()
        } else {
            String::new()
        };

        let expected_stdout = if let Some(s) = &case.stdout {
            s.clone()
        } else if let Some(f) = &case.stdout_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap()
        } else {
            panic!(
                "IoTestCase must specify stdout or stdout_file (test: {}, case: {})",
                test_name, case_name
            );
        };

        let mut vm = WhitespaceVM::from_source(&ws_code)
            .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e));

        let stdin_cursor: Box<dyn io::BufRead> =
            Box::new(io::Cursor::new(stdin_content.into_bytes()));
        let stdout_buf: Box<dyn io::Write> = Box::new(Vec::<u8>::new());
        vm = vm.with_io(stdin_cursor, stdout_buf);

        let result = vm.run(1_000_000);

        match result {
            StepResult::Complete => {}
            StepResult::Suspended => panic!(
                "Whitespace execution suspended (exceeded step limit) for {}, case '{}'",
                test_name, case_name
            ),
            StepResult::Error(e) => panic!(
                "Whitespace execution failed for {}, case '{}': {:?}",
                test_name, case_name, e
            ),
            StepResult::WaitingForInput(t) => panic!(
                "Whitespace execution unexpectedly waiting for input ({:?}) for {}, case '{}'",
                t, test_name, case_name
            ),
        }

        let actual_stdout = vm.get_stdout_string();

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'
Expected: {:?}
Actual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }
}

use super::test_config::TestConfig;

#[allow(dead_code)]
pub fn test_whitespace_self_base(test_name: &str) {
    test_whitespace_self_base_debug(test_name, false);
}

pub fn test_whitespace_self_base_debug(test_name: &str, debug_ext: bool) {
    test_whitespace_self_base_impl(test_name, debug_ext, false, false);
}

pub fn test_whitespace_self_base_strict(test_name: &str, debug_ext: bool) {
    test_whitespace_self_base_impl(test_name, debug_ext, false, true);
}

pub fn test_whitespace_self_base_randomize(test_name: &str, debug_ext: bool) {
    test_whitespace_self_base_impl(test_name, debug_ext, true, false);
}

#[allow(dead_code)]
pub fn test_whitespace_self_base_alloc(test_name: &str, debug_ext: bool, alloc_ext: bool) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    // コンパイル（alloc_ext を渡す）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace_with_options(&a, debug_ext, alloc_ext)
        .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

    // Whitespace コードが空白文字のみであることを確認
    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );

    // 独自 WhitespaceVM で実行
    let mut vm = WhitespaceVM::from_source(&ws_code)
        .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e))
        .with_debug_ext(debug_ext);

    let result = vm.run(1_000_000);

    match result {
        StepResult::Complete => {}
        StepResult::Suspended => panic!(
            "Whitespace execution suspended (exceeded step limit) for {}",
            test_name
        ),
        StepResult::Error(e) => panic!("Whitespace execution failed for {}: {:?}", test_name, e),
        StepResult::WaitingForInput(t) => panic!(
            "Whitespace execution unexpectedly waiting for input ({:?}) for {}",
            t, test_name
        ),
    }
}

fn test_whitespace_self_base_impl(
    test_name: &str,
    debug_ext: bool,
    randomize_heap: bool,
    strict_heap: bool,
) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    // コンパイル
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace_with_options(&a, debug_ext, false)
        .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

    // Whitespace コードが空白文字のみであることを確認
    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );

    // 独自 WhitespaceVM で実行
    let mut vm = WhitespaceVM::from_source(&ws_code)
        .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e))
        .with_debug_ext(debug_ext)
        .with_strict_heap(strict_heap)
        .with_randomize_heap(randomize_heap);

    let result = vm.run(1_000_000);

    match result {
        StepResult::Complete => {}
        StepResult::Suspended => panic!(
            "Whitespace execution suspended (exceeded step limit) for {}",
            test_name
        ),
        StepResult::Error(e) => panic!("Whitespace execution failed for {}: {:?}", test_name, e),
        StepResult::WaitingForInput(t) => panic!(
            "Whitespace execution unexpectedly waiting for input ({:?}) for {}",
            t, test_name
        ),
    }
}

#[allow(dead_code)]
pub fn test_whitespace_self_io_base(test_name: &str) {
    test_whitespace_self_io_base_debug(test_name, false);
}

pub fn test_whitespace_self_io_base_debug(test_name: &str, debug_ext: bool) {
    test_whitespace_self_io_base_impl(test_name, debug_ext, false, false);
}

pub fn test_whitespace_self_io_base_strict(test_name: &str, debug_ext: bool) {
    test_whitespace_self_io_base_impl(test_name, debug_ext, false, true);
}

pub fn test_whitespace_self_io_base_randomize(test_name: &str, debug_ext: bool) {
    test_whitespace_self_io_base_impl(test_name, debug_ext, true, false);
}

pub fn test_whitespace_self_io_base_alloc(test_name: &str, debug_ext: bool, alloc_ext: bool) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let check_json_value: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();

    let check_json: TestConfig = serde_json::from_value(check_json_value).ok().unwrap();
    let test_cases = check_json.get_io_test_cases();

    // コンパイル（alloc_ext を渡す）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace_with_options(&a, debug_ext, alloc_ext)
        .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

    // Whitespace コードが空白文字のみであることを確認
    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );

    // 各ケースを実行
    for (idx, case) in test_cases.iter().enumerate() {
        let case_name = case
            .name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format!("case_{}", idx));

        // stdin を取得
        let stdin_content = if let Some(s) = &case.stdin {
            s.clone()
        } else if let Some(f) = &case.stdin_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap_or_default()
        } else {
            String::new()
        };

        // 期待される stdout を取得
        let expected_stdout = if let Some(s) = &case.stdout {
            s.clone()
        } else if let Some(f) = &case.stdout_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap()
        } else {
            panic!(
                "IoTestCase must specify stdout or stdout_file (test: {}, case: {})",
                test_name, case_name
            );
        };

        // 独自 WhitespaceVM で実行
        let mut vm = WhitespaceVM::from_source(&ws_code)
            .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e))
            .with_debug_ext(debug_ext);

        let stdin_cursor: Box<dyn io::BufRead> =
            Box::new(io::Cursor::new(stdin_content.into_bytes()));
        let stdout_buf: Box<dyn io::Write> = Box::new(Vec::<u8>::new());
        vm = vm.with_io(stdin_cursor, stdout_buf);

        let result = vm.run(1_000_000);

        match result {
            StepResult::Complete => {}
            StepResult::Suspended => panic!(
                "Whitespace execution suspended (exceeded step limit) for {}, case '{}'",
                test_name, case_name
            ),
            StepResult::Error(e) => panic!(
                "Whitespace execution failed for {}, case '{}': {:?}",
                test_name, case_name, e
            ),
            StepResult::WaitingForInput(t) => panic!(
                "Whitespace execution unexpectedly waiting for input ({:?}) for {}, case '{}'",
                t, test_name, case_name
            ),
        }

        let actual_stdout = vm.get_stdout_string();

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'\nExpected: {:?}\nActual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }
}

fn test_whitespace_self_io_base_impl(
    test_name: &str,
    debug_ext: bool,
    randomize_heap: bool,
    strict_heap: bool,
) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let check_json_value: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();

    let check_json: TestConfig = serde_json::from_value(check_json_value).ok().unwrap();
    let test_cases = check_json.get_io_test_cases();

    // コンパイル（全ケース共通）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace_with_options(&a, debug_ext, false)
        .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

    // Whitespace コードが空白文字のみであることを確認
    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );

    // 各ケースを実行
    for (idx, case) in test_cases.iter().enumerate() {
        let case_name = case
            .name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format!("case_{}", idx));

        // stdin を取得
        let stdin_content = if let Some(s) = &case.stdin {
            s.clone()
        } else if let Some(f) = &case.stdin_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap_or_default()
        } else {
            String::new()
        };

        // 期待される stdout を取得
        let expected_stdout = if let Some(s) = &case.stdout {
            s.clone()
        } else if let Some(f) = &case.stdout_file {
            fs::read_to_string(path_base.clone() + "." + f).unwrap()
        } else {
            panic!(
                "IoTestCase must specify stdout or stdout_file (test: {}, case: {})",
                test_name, case_name
            );
        };

        // 独自 WhitespaceVM で実行
        let mut vm = WhitespaceVM::from_source(&ws_code)
            .unwrap_or_else(|e| panic!("Failed to parse Whitespace for {}: {:?}", test_name, e))
            .with_debug_ext(debug_ext)
            .with_strict_heap(strict_heap)
            .with_randomize_heap(randomize_heap);

        let stdin_cursor: Box<dyn io::BufRead> =
            Box::new(io::Cursor::new(stdin_content.into_bytes()));
        let stdout_buf: Box<dyn io::Write> = Box::new(Vec::<u8>::new());
        vm = vm.with_io(stdin_cursor, stdout_buf);

        let result = vm.run(1_000_000);

        match result {
            StepResult::Complete => {}
            StepResult::Suspended => panic!(
                "Whitespace execution suspended (exceeded step limit) for {}, case '{}'",
                test_name, case_name
            ),
            StepResult::Error(e) => panic!(
                "Whitespace execution failed for {}, case '{}': {:?}",
                test_name, case_name, e
            ),
            StepResult::WaitingForInput(t) => panic!(
                "Whitespace execution unexpectedly waiting for input ({:?}) for {}, case '{}'",
                t, test_name, case_name
            ),
        }

        let actual_stdout = vm.get_stdout_string();

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'\nExpected: {:?}\nActual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }
}
