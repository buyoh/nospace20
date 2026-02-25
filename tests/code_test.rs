use std::{fmt::Result, fs, io};

mod common;

use nospace20::whitespace::{StepResult, WhitespaceVM};
use nospace20::{
    compile_to_whitespace, compile_to_whitespace_with_options, interpret_func_testing,
    interpret_func_testing_randomize, interpret_func_with_io, parse_to_tokens, parse_to_tree,
    syntactic_analyze,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct IoTestCase {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    stdin: Option<String>,
    #[serde(default)]
    stdin_file: Option<String>,
    #[serde(default)]
    stdout: Option<String>,
    #[serde(default)]
    stdout_file: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum TestConfig {
    Success {
        #[serde(alias = "trace")]
        trace_hit_counts: Vec<i64>,
    },
    SuccessIo {
        // 後方互換性のため残す（cases が未定義の場合に使用）
        #[serde(default)]
        stdin: Option<String>,
        #[serde(default)]
        stdin_file: Option<String>,
        #[serde(default)]
        stdout: Option<String>,
        #[serde(default)]
        stdout_file: Option<String>,
        // 新規追加: 複数ケースのサポート
        #[serde(default)]
        cases: Option<Vec<IoTestCase>>,
    },
    ParseError {
        phase: String, // "tokenize" or "tree"
        #[serde(skip_serializing_if = "Option::is_none")]
        error_count: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
    CompileError {
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
    RuntimeError {
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
}

// 後方互換性のため、"trace" フィールドのみの場合は Success として扱う
impl TestConfig {
    fn from_legacy(value: &serde_json::Value) -> Option<Self> {
        if value.get("type").is_none()
            && (value.get("trace").is_some() || value.get("trace_hit_counts").is_some())
        {
            let trace_hit_counts = value
                .get("trace_hit_counts")
                .or_else(|| value.get("trace"))?
                .as_array()?
                .iter()
                .map(|e| e.as_i64().unwrap())
                .collect();
            Some(TestConfig::Success { trace_hit_counts })
        } else {
            None
        }
    }

    /// SuccessIo テストから IoTestCase のリストを取得
    /// 後方互換性のため、cases が未定義の場合は従来のフィールドから1ケースを作成
    fn get_io_test_cases(&self) -> Vec<IoTestCase> {
        match self {
            TestConfig::SuccessIo {
                stdin,
                stdin_file,
                stdout,
                stdout_file,
                cases,
            } => {
                if let Some(cases) = cases {
                    // 新形式: cases が定義されている
                    cases.clone()
                } else {
                    // 旧形式: cases が未定義の場合、従来のフィールドから1ケースを作成
                    vec![IoTestCase {
                        name: Some("default".to_string()),
                        stdin: stdin.clone(),
                        stdin_file: stdin_file.clone(),
                        stdout: stdout.clone(),
                        stdout_file: stdout_file.clone(),
                    }]
                }
            }
            _ => panic!("Not a SuccessIo test config"),
        }
    }
}

fn test_ok_coding_base(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = syntactic_analyze(&s).ok().unwrap();
    let trace = interpret_func_testing(&a, "main");

    let check_json_value: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();

    // 後方互換性: "trace" フィールドのみの場合
    let check_json = if let Some(legacy) = TestConfig::from_legacy(&check_json_value) {
        legacy
    } else {
        match serde_json::from_value(check_json_value.clone()) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to parse config: {:?}", e);
                eprintln!("JSON value: {:?}", check_json_value);
                panic!("Failed to parse test config");
            }
        }
    };

    match check_json {
        TestConfig::Success {
            trace_hit_counts: expected_trace_hit_counts,
        } => {
            let expected_iter = expected_trace_hit_counts.into_iter();
            for (i, expected) in expected_iter.enumerate() {
                let key = i as i64;
                if let Some(actual) = trace.get(&key) {
                    assert_eq!(expected, *actual, "trace(idx:{}) failed", key);
                } else {
                    panic!("idx:{} trace doesn't exist", key);
                }
            }
        }
        _ => panic!("Expected success test config"),
    }
    Ok(())
}

/// インタプリタのランダム初期化モードで success テストを実行する
///
/// 未初期化変数にランダム値を設定して実行し、trace の結果が一致するか確認する。
/// 初期値 0 に依存したコードの場合は失敗する。
fn test_ok_coding_base_randomize(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = syntactic_analyze(&s).ok().unwrap();
    // randomize モードで実行
    let trace = interpret_func_testing_randomize(&a, "main");

    let check_json_value: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();

    let check_json = if let Some(legacy) = TestConfig::from_legacy(&check_json_value) {
        legacy
    } else {
        serde_json::from_value(check_json_value).expect("Failed to parse test config")
    };

    match check_json {
        TestConfig::Success {
            trace_hit_counts: expected_trace_hit_counts,
        } => {
            let expected_iter = expected_trace_hit_counts.into_iter();
            for (i, expected) in expected_iter.enumerate() {
                let key = i as i64;
                if let Some(actual) = trace.get(&key) {
                    assert_eq!(expected, *actual, "trace(idx:{}) failed", key);
                } else {
                    panic!("idx:{} trace doesn't exist", key);
                }
            }
        }
        _ => panic!("Expected success test config"),
    }
    Ok(())
}

fn test_ok_coding_io_base(test_name: &str) -> Result {
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

    // パース（全ケース共通）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();

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

        // 実行
        let (_, actual_stdout) = interpret_func_with_io(&a, "main", &stdin_content);

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'\nExpected: {:?}\nActual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }

    Ok(())
}

fn test_syntax_error_base(test_name: &str) -> Result {
    let path_base = "resources/tests/fails/syntax/".to_owned() + test_name;
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

    match check_json {
        TestConfig::ParseError {
            phase,
            error_count: _,
            contains: _,
        } => match phase.as_str() {
            "tokenize" => {
                let result = parse_to_tokens(&ns_cnt);
                assert!(result.is_err(), "Expected tokenize error but succeeded");
            }
            "tree" => {
                let t = parse_to_tokens(&ns_cnt).ok().unwrap();
                let result = parse_to_tree(&t);
                assert!(result.is_err(), "Expected tree parse error but succeeded");
            }
            _ => panic!("Unknown phase: {}", phase),
        },
        _ => panic!("Expected parse_error test config"),
    }
    Ok(())
}

fn test_compile_error_base(test_name: &str) -> Result {
    let path_base = "resources/tests/fails/compile/".to_owned() + test_name;
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

    match check_json {
        TestConfig::CompileError { contains } => {
            // パース
            let t = parse_to_tokens(&ns_cnt).ok().unwrap();
            let s = parse_to_tree(&t).ok().unwrap();

            // セマンティック分析（エラーが発生する可能性がある）
            let a = match syntactic_analyze(&s) {
                Ok(a) => a,
                Err(errors) => {
                    // セマンティック分析でエラーが発生した場合もチェック
                    if let Some(keywords) = &contains {
                        // すべてのエラーメッセージを結合
                        let error_messages: Vec<String> =
                            errors.iter().map(|e| e.message.to_string()).collect();
                        let combined_errors = error_messages.join("\n");

                        for keyword in keywords {
                            assert!(
                                combined_errors.contains(keyword),
                                "Semantic error message does not contain '{}': {}",
                                keyword,
                                combined_errors
                            );
                        }
                    }
                    return Ok(());
                }
            };

            // コンパイル（エラーが発生するはず）
            let result = compile_to_whitespace(&a);
            assert!(result.is_err(), "Expected compile error but succeeded");

            // contains が指定されている場合、エラーメッセージに含まれているか確認
            if let Some(keywords) = contains {
                let error_msg = result.unwrap_err();
                for keyword in keywords {
                    assert!(
                        error_msg.contains(&keyword),
                        "Error message does not contain '{}': {}",
                        keyword,
                        error_msg
                    );
                }
            }
        }
        _ => panic!("Expected compile_error test config"),
    }
    Ok(())
}

fn test_runtime_error_base(test_name: &str) -> Result {
    let path_base = "resources/tests/fails/runtime/".to_owned() + test_name;
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

    match check_json {
        TestConfig::RuntimeError { contains } => {
            // パース
            let t = parse_to_tokens(&ns_cnt).ok().unwrap();
            let s = parse_to_tree(&t).ok().unwrap();
            let a = syntactic_analyze(&s).ok().unwrap();

            // 実行してパニックをキャッチ
            let result = std::panic::catch_unwind(|| {
                interpret_func_with_io(&a, "main", "");
            });

            assert!(result.is_err(), "Expected runtime panic but succeeded");

            // contains が指定されている場合、パニックメッセージに含まれているか確認
            if let Some(keywords) = contains {
                if let Err(panic_info) = result {
                    // パニックメッセージを取得
                    let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        String::new()
                    };

                    for keyword in keywords {
                        assert!(
                            panic_msg.contains(&keyword),
                            "Panic message does not contain '{}': {}",
                            keyword,
                            panic_msg
                        );
                    }
                }
            }
        }
        _ => panic!("Expected runtime_error test config"),
    }
    Ok(())
}

fn test_whitespace_base(test_name: &str) {
    use common::{run_whitespace, wsc_available};

    // wsc が利用できない場合はスキップ
    if !wsc_available() {
        eprintln!("Skipping test: wsc not available");
        eprintln!("Run: ./tools/setup-wsc.sh");
        return;
    }

    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    // コンパイル
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace(&a).unwrap_or_else(|e| panic!("Compilation failed: {}", e));

    // Whitespace コードが空白文字のみであることを確認
    assert!(!ws_code.is_empty(), "Whitespace code is empty");
    assert!(
        ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'),
        "Whitespace code contains non-whitespace characters"
    );

    // whitespace 実行（__trace は無視され、実行が成功すればOK）
    let result = run_whitespace(&ws_code, "");

    // 実行エラーがあればパニック
    if let Err(e) = result {
        panic!("Whitespace execution failed for {}: {}", test_name, e);
    }
}

fn test_whitespace_io_base(test_name: &str) {
    use common::{run_whitespace, wsc_available};

    // wsc が利用できない場合はスキップ
    if !wsc_available() {
        eprintln!("Skipping test: wsc not available");
        eprintln!("Run: ./tools/setup-wsc.sh");
        return;
    }

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
    let ws_code = compile_to_whitespace(&a).unwrap_or_else(|e| panic!("Compilation failed: {}", e));

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

        // whitespace 実行
        let actual_stdout = run_whitespace(&ws_code, &stdin_content)
            .unwrap_or_else(|e| panic!("Whitespace execution failed: {}", e));

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'\nExpected: {:?}\nActual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }
}

#[allow(dead_code)]
fn test_whitespace_self_base(test_name: &str) {
    test_whitespace_self_base_debug(test_name, false);
}

fn test_whitespace_self_base_debug(test_name: &str, debug_ext: bool) {
    test_whitespace_self_base_impl(test_name, debug_ext, false, false);
}

fn test_whitespace_self_base_strict(test_name: &str, debug_ext: bool) {
    test_whitespace_self_base_impl(test_name, debug_ext, false, true);
}

fn test_whitespace_self_base_randomize(test_name: &str, debug_ext: bool) {
    test_whitespace_self_base_impl(test_name, debug_ext, true, false);
}

fn test_whitespace_self_base_alloc(test_name: &str, debug_ext: bool, alloc_ext: bool) {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    // コンパイル（alloc_ext を渡す）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace_with_options(&a, debug_ext, alloc_ext)
        .unwrap_or_else(|e| panic!("Compilation failed: {}", e));

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
        .unwrap_or_else(|e| panic!("Compilation failed: {}", e));

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
fn test_whitespace_self_io_base(test_name: &str) {
    test_whitespace_self_io_base_debug(test_name, false);
}

fn test_whitespace_self_io_base_debug(test_name: &str, debug_ext: bool) {
    test_whitespace_self_io_base_impl(test_name, debug_ext, false, false);
}

fn test_whitespace_self_io_base_strict(test_name: &str, debug_ext: bool) {
    test_whitespace_self_io_base_impl(test_name, debug_ext, false, true);
}

fn test_whitespace_self_io_base_randomize(test_name: &str, debug_ext: bool) {
    test_whitespace_self_io_base_impl(test_name, debug_ext, true, false);
}

fn test_whitespace_self_io_base_alloc(test_name: &str, debug_ext: bool, alloc_ext: bool) {
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
        .unwrap_or_else(|e| panic!("Compilation failed: {}", e));

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
        .unwrap_or_else(|e| panic!("Compilation failed: {}", e));

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

// ========================================
// 自動生成されたテスト
// ========================================
// テストは resources/tests/test-manifest.yaml で定義され、
// build.rs によって自動生成されます。
// テストを追加する場合は test-manifest.yaml を編集してください。

include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
