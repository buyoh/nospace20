use std::{fs, io};

use nospace20::{
    compile_to_ws, parse_to_tokens, parse_to_tree, semantic_analyze, WsCompileOptions,
};

use nospace20::whitespace::StepResult;

use super::test_config::{load_check_json, TestConfig};
use super::whitespace_self_base::{
    compile_for_ws_self, new_ws_self_vm, WsSelfTarget, WS_SELF_MAX_STEPS,
};

pub fn test_syntax_error_base(test_name: &str) {
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
            "tree" | "parse" => {
                // "parse" は "tree" 相当のフェーズ（旧名称）として扱う
                let t = parse_to_tokens(&ns_cnt).ok().unwrap();
                let result = parse_to_tree(&t);
                assert!(result.is_err(), "Expected tree parse error but succeeded");
            }
            _ => panic!("Unknown phase: {}", phase),
        },
        _ => panic!("Expected parse_error test config"),
    }
}

pub fn test_compile_error_base(test_name: &str) {
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
            let a = match semantic_analyze(&s) {
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
                    return;
                }
            };

            // コンパイル（エラーが発生するはず）
            let result = compile_to_ws(&a, &WsCompileOptions::default());
            assert!(result.is_err(), "Expected compile error but succeeded");

            // contains が指定されている場合、エラーメッセージに含まれているか確認
            if let Some(keywords) = contains {
                let error = result.unwrap_err();
                let combined_msg = error.kind.to_string();
                for keyword in keywords {
                    assert!(
                        combined_msg.contains(keyword.as_str()),
                        "Error message does not contain '{}': {}",
                        keyword,
                        combined_msg
                    );
                }
            }
        }
        _ => panic!("Expected compile_error test config"),
    }
}

/// runtime_error テストを compile → 独自 WhitespaceVM で実行し、実行時エラーで止まることを確かめる
///
/// check.json の `contains` は WhitespaceVM のエラー文言（`WsRuntimeError` の Display）と照合する。
/// 意図しないエラー（StackUnderflow など、コンパイラの不具合によるもの）で止まった場合に失敗させるため
pub fn test_runtime_error_ws_self_base(test_name: &str, target: WsSelfTarget) {
    let path_base = "resources/tests/fails/runtime/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let contains = match load_check_json(&path_base) {
        TestConfig::RuntimeError { contains } => contains.unwrap_or_default(),
        _ => panic!("Expected runtime_error test config"),
    };

    let ws_code = compile_for_ws_self(&ns_cnt, target);
    let mut vm = new_ws_self_vm(&ws_code, target, test_name);
    let result = vm.run(WS_SELF_MAX_STEPS);
    let StepResult::Error(err) = result else {
        panic!(
            "Expected runtime error but got {:?} (test: {})",
            result, test_name
        );
    };
    let message = err.to_string();
    for keyword in contains {
        assert!(
            message.contains(&keyword),
            "Runtime error message does not contain '{}': {} (test: {})",
            keyword,
            message,
            test_name
        );
    }
}
