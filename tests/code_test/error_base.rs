use std::{fmt::Result, fs, io};

use nospace20::{
    compile_to_whitespace, interpret_func_with_io, parse_to_tokens, parse_to_tree,
    syntactic_analyze,
};

use super::test_config::TestConfig;

pub fn test_syntax_error_base(test_name: &str) -> Result {
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

pub fn test_compile_error_base(test_name: &str) -> Result {
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
                let errors = result.unwrap_err();
                let combined_msg = errors
                    .iter()
                    .map(|e| e.message.as_ref())
                    .collect::<Vec<_>>()
                    .join("\n");
                for keyword in keywords {
                    assert!(
                        combined_msg.contains(&keyword as &str),
                        "Error message does not contain '{}': {}",
                        keyword,
                        combined_msg
                    );
                }
            }
        }
        _ => panic!("Expected compile_error test config"),
    }
    Ok(())
}

pub fn test_runtime_error_base(test_name: &str) -> Result {
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
