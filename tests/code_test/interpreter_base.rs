use std::{fmt::Result, fs, io};

use nospace20::{
    interpret_func_testing, interpret_func_testing_randomize, interpret_func_with_io,
    parse_to_tokens, parse_to_tree, syntactic_analyze,
};

use super::test_config::{load_check_json, TestConfig};

/// `--opt all` 相当の最適化を適用した上でインタプリタ実行する
pub fn test_ok_coding_base_opt_all(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let mut a = syntactic_analyze(&s).ok().unwrap();
    nospace20::optimize(&mut a, &nospace20::OptimizationOptions::all());
    let trace = interpret_func_testing(&a, "main");

    let check_json = load_check_json(&path_base);

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

/// `--opt all` 相当の最適化を適用した上で IO テストをインタプリタ実行する
pub fn test_ok_coding_io_base_opt_all(test_name: &str) -> Result {
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

    // パース・最適化（全ケース共通）
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let mut a = syntactic_analyze(&s).unwrap();
    nospace20::optimize(&mut a, &nospace20::OptimizationOptions::all());

    // 各ケースを実行
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

        let (_, actual_stdout) = interpret_func_with_io(&a, "main", &stdin_content);

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in test '{}', case '{}'
Expected: {:?}
Actual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }

    Ok(())
}

pub fn test_ok_coding_base(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = syntactic_analyze(&s).ok().unwrap();
    let trace = interpret_func_testing(&a, "main");

    let check_json = load_check_json(&path_base);

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
pub fn test_ok_coding_base_randomize(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = syntactic_analyze(&s).ok().unwrap();
    // randomize モードで実行
    let trace = interpret_func_testing_randomize(&a, "main");

    let check_json = load_check_json(&path_base);

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

pub fn test_ok_coding_io_base(test_name: &str) -> Result {
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
