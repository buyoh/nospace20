use std::{fmt::Result, fs, io};

use nospace20::{
    interpret_func_testing, interpret_func_with_io, parse_to_tokens, parse_to_tree,
    syntactic_analyze,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum TestConfig {
    Success {
        trace: Vec<i64>,
    },
    SuccessIo {
        #[serde(default)]
        stdin: Option<String>,
        #[serde(default)]
        stdin_file: Option<String>,
        stdout: Option<String>,
        stdout_file: Option<String>,
    },
    ParseError {
        phase: String, // "tokenize" or "tree"
        #[serde(skip_serializing_if = "Option::is_none")]
        error_count: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<Vec<String>>,
    },
}

// 後方互換性のため、"trace" フィールドのみの場合は Success として扱う
impl TestConfig {
    fn from_legacy(value: &serde_json::Value) -> Option<Self> {
        if value.get("type").is_none() && value.get("trace").is_some() {
            let trace = value
                .get("trace")?
                .as_array()?
                .iter()
                .map(|e| e.as_i64().unwrap())
                .collect();
            Some(TestConfig::Success { trace })
        } else {
            None
        }
    }
}

fn test_ok_coding_base(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = syntactic_analyze(&s);
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
            trace: expected_trace_vec,
        } => {
            let expected_trace = expected_trace_vec.into_iter();
            for (i, expected) in expected_trace.enumerate() {
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

    match check_json {
        TestConfig::SuccessIo {
            stdin,
            stdin_file,
            stdout,
            stdout_file,
        } => {
            // stdin を取得（インラインまたはファイルから）
            let stdin_content = if let Some(s) = stdin {
                s
            } else if let Some(f) = stdin_file {
                fs::read_to_string(path_base.clone() + "." + &f).unwrap_or_default()
            } else {
                String::new()
            };

            // 期待される stdout を取得
            let expected_stdout = if let Some(s) = stdout {
                s
            } else if let Some(f) = stdout_file {
                fs::read_to_string(path_base.clone() + "." + &f).unwrap()
            } else {
                panic!("SuccessIo test must specify stdout or stdout_file");
            };

            // 実行
            let t = parse_to_tokens(&ns_cnt).unwrap();
            let s = parse_to_tree(&t).unwrap();
            let a = syntactic_analyze(&s);
            let (_, actual_stdout) = interpret_func_with_io(&a, "main", &stdin_content);

            assert_eq!(expected_stdout, actual_stdout, "stdout mismatch");
        }
        _ => panic!("Expected success_io test config"),
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

// ========================================
// 自動生成されたテスト
// ========================================
// テストは resources/tests/test-manifest.yaml で定義され、
// build.rs によって自動生成されます。
// テストを追加する場合は test-manifest.yaml を編集してください。

include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
