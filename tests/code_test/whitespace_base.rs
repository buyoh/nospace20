use std::{fs, io};

use nospace20::{
    compile_to_ws, parse_to_tokens, parse_to_tree, semantic_analyze, WsCompileOptions,
};

use super::test_config::TestConfig;

pub fn test_whitespace_base(test_name: &str) {
    use crate::common::{run_whitespace, wsc_available};

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
    let a = semantic_analyze(&s).unwrap();
    let ws_code = compile_to_ws(&a, &WsCompileOptions::default())
        .unwrap_or_else(|e| panic!("Compilation failed: {:?}", e));

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

pub fn test_whitespace_io_base(test_name: &str) {
    use crate::common::{run_whitespace, wsc_available};

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
    let a = semantic_analyze(&s).unwrap();
    let ws_code = compile_to_ws(&a, &WsCompileOptions::default())
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
