//! Whitespace 直接テスト用のベース関数

use nospace20::whitespace::{ParseError, RuntimeError, StepResult, WhitespaceVM};
use std::fs;
use std::io;

/// WSA 形式をデコード（S, T, N のみを抽出、# で始まる行は除外）
fn decode_wsa(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.trim().starts_with('#'))
        .flat_map(|line| line.chars())
        .filter_map(|c| match c {
            'S' => Some(' '),
            'T' => Some('\t'),
            'N' => Some('\n'),
            _ => None,
        })
        .collect()
}

/// ws_io テスト: 正常系の I/O 検証
pub fn test_ws_io_base(test_name: &str) {
    let path_base = format!("resources/tests_ws/passes/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base))
        .expect(&format!("Failed to read {}.wsa", test_name));
    let ws_code = decode_wsa(&wsa_content);

    let check: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(format!("{}.check.json", path_base))
            .expect(&format!("Failed to read {}.check.json", test_name)),
    ))
    .expect(&format!("Failed to parse check.json for {}", test_name));

    let stdin_str = check.get("stdin").and_then(|v| v.as_str()).unwrap_or("");
    let expected_stdout = check
        .get("stdout")
        .and_then(|v| v.as_str())
        .expect(&format!(
            "No 'stdout' field in check.json for {}",
            test_name
        ));

    let vm = WhitespaceVM::from_source(&ws_code)
        .expect(&format!("Failed to parse Whitespace for {}", test_name));

    let stdin_cursor: Box<dyn io::BufRead> =
        Box::new(io::Cursor::new(stdin_str.to_string().into_bytes()));

    let mut vm = vm.with_stdin(stdin_cursor);
    let result = vm.run(100_000);

    assert_eq!(
        result,
        StepResult::Complete,
        "Test {} did not complete: {:?}",
        test_name,
        result
    );

    let actual_stdout = vm.get_stdout_string();
    assert_eq!(
        expected_stdout, actual_stdout,
        "Test {} stdout mismatch",
        test_name
    );
}

/// ws_runtime_error テスト: 実行時エラー検証
pub fn test_ws_runtime_error_base(test_name: &str) {
    let path_base = format!("resources/tests_ws/fails/runtime/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base))
        .expect(&format!("Failed to read {}.wsa", test_name));
    let ws_code = decode_wsa(&wsa_content);

    let check: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(format!("{}.check.json", path_base))
            .expect(&format!("Failed to read {}.check.json", test_name)),
    ))
    .expect(&format!("Failed to parse check.json for {}", test_name));

    let expected_error = check
        .get("error")
        .and_then(|v| v.as_str())
        .expect(&format!("No 'error' field in check.json for {}", test_name));

    let mut vm = WhitespaceVM::from_source(&ws_code)
        .expect(&format!("Failed to parse Whitespace for {}", test_name));

    let result = vm.run(100_000);

    match result {
        StepResult::Error(e) => {
            let error_name = match e {
                RuntimeError::StackUnderflow => "StackUnderflow",
                RuntimeError::DivisionByZero => "DivisionByZero",
                RuntimeError::UndefinedLabel(_) => "UndefinedLabel",
                RuntimeError::UninitializedHeap(_) => "UninitializedHeap",
                RuntimeError::CallStackUnderflow => "CallStackUnderflow",
                RuntimeError::ProgramCounterOutOfBounds => "ProgramCounterOutOfBounds",
                RuntimeError::IoError(_) => "IoError",
                RuntimeError::AssertionFailed(_) => "AssertionFailed",
            };
            assert_eq!(
                expected_error, error_name,
                "Test {} error type mismatch",
                test_name
            );
        }
        _ => panic!("Test {} expected error but got: {:?}", test_name, result),
    }
}

/// ws_parse_error テスト: パースエラー検証
pub fn test_ws_parse_error_base(test_name: &str) {
    let path_base = format!("resources/tests_ws/fails/parse/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base))
        .expect(&format!("Failed to read {}.wsa", test_name));
    let ws_code = decode_wsa(&wsa_content);

    let check: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(format!("{}.check.json", path_base))
            .expect(&format!("Failed to read {}.check.json", test_name)),
    ))
    .expect(&format!("Failed to parse check.json for {}", test_name));

    let expected_error = check
        .get("error")
        .and_then(|v| v.as_str())
        .expect(&format!("No 'error' field in check.json for {}", test_name));

    let result = WhitespaceVM::from_source(&ws_code);

    match result {
        Err(e) => {
            let error_name = match e {
                ParseError::DuplicateLabel { .. } => "DuplicateLabel",
                ParseError::InvalidImp { .. } => "InvalidImp",
                ParseError::InvalidCommand { .. } => "InvalidCommand",
                ParseError::UnexpectedEof { .. } => "UnexpectedEof",
                ParseError::InvalidNumber { .. } => "InvalidNumber",
                ParseError::InvalidLabel { .. } => "InvalidLabel",
            };
            assert_eq!(
                expected_error, error_name,
                "Test {} error type mismatch",
                test_name
            );
        }
        Ok(_) => panic!(
            "Test {} expected parse error but parsing succeeded",
            test_name
        ),
    }
}

/// ws_io テスト (wsc クロスバリデーション): 正常系の I/O 検証
pub fn test_ws_io_wsc_base(test_name: &str) {
    if !crate::common::wsc_available() {
        eprintln!("Skipping test: wsc not available");
        eprintln!("Run: ./tools/setup-wsc.sh");
        return;
    }

    let path_base = format!("resources/tests_ws/passes/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base))
        .expect(&format!("Failed to read {}.wsa", test_name));
    let ws_code = decode_wsa(&wsa_content);

    let check: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(format!("{}.check.json", path_base))
            .expect(&format!("Failed to read {}.check.json", test_name)),
    ))
    .expect(&format!("Failed to parse check.json for {}", test_name));

    let stdin_str = check.get("stdin").and_then(|v| v.as_str()).unwrap_or("");
    let expected_stdout = check
        .get("stdout")
        .and_then(|v| v.as_str())
        .expect(&format!(
            "No 'stdout' field in check.json for {}",
            test_name
        ));

    // wsc で実行
    let actual_stdout = crate::common::run_whitespace(&ws_code, stdin_str)
        .expect(&format!("wsc failed for test {}", test_name));

    assert_eq!(
        expected_stdout, actual_stdout,
        "Test {} stdout mismatch (wsc)",
        test_name
    );
}

/// ws_runtime_error テスト (wsc クロスバリデーション): 実行時エラー検証
pub fn test_ws_runtime_error_wsc_base(test_name: &str) {
    if !crate::common::wsc_available() {
        eprintln!("Skipping test: wsc not available");
        eprintln!("Run: ./tools/setup-wsc.sh");
        return;
    }

    let path_base = format!("resources/tests_ws/fails/runtime/{}", test_name);
    let wsa_content = fs::read_to_string(format!("{}.wsa", path_base))
        .expect(&format!("Failed to read {}.wsa", test_name));
    let ws_code = decode_wsa(&wsa_content);

    // wsc で実行（エラーを期待）
    let result = crate::common::run_whitespace(&ws_code, "");

    assert!(
        result.is_err(),
        "Test {} expected error but wsc succeeded",
        test_name
    );
}
