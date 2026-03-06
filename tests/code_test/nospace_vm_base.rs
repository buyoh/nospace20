use std::{fmt::Result, fs, io};

use nospace20::{parse_to_tokens, parse_to_tree, semantic_analyze, NospaceVM, StepResult};

use super::test_config::{load_check_json, TestConfig};

/// NospaceVM を使って success テストを実行する
///
/// 既存の再帰インタプリタ (`test_ok_coding_base`) と同等のテストを
/// NospaceVM のステップ実行で行う。trace の結果が一致することを確認する。
pub fn test_ok_coding_base_vm(test_name: &str) -> Result {
    let path_base = "resources/tests/passes/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = semantic_analyze(&s).ok().unwrap();

    let mut vm = NospaceVM::from_scope(a).expect("failed to create NospaceVM");
    let result = vm.run(1_000_000);
    match result {
        StepResult::Complete { .. } => {}
        StepResult::Suspended => panic!("NospaceVM: did not complete within budget"),
        StepResult::Error(e) => panic!("NospaceVM runtime error: {:?}", e),
    }

    let check_json = load_check_json(&path_base);

    match check_json {
        TestConfig::Success {
            trace_hit_counts: expected_trace_hit_counts,
        } => {
            let trace = vm.traced();
            let expected_iter = expected_trace_hit_counts.into_iter();
            for (i, expected) in expected_iter.enumerate() {
                let key = i as i64;
                if let Some(actual) = trace.get(&key) {
                    assert_eq!(expected, *actual, "trace(idx:{}) failed (NospaceVM)", key);
                } else {
                    panic!("idx:{} trace doesn't exist (NospaceVM)", key);
                }
            }
        }
        _ => panic!("Expected success test config"),
    }
    Ok(())
}

/// NospaceVM を使って success_io テストを実行する
///
/// 既存の再帰インタプリタ (`test_ok_coding_io_base`) と同等のテストを
/// NospaceVM のステップ実行で行う。stdout の結果が一致することを確認する。
pub fn test_ok_coding_io_base_vm(test_name: &str) -> Result {
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

        // 毎ケースごとに Scope を再生成（NospaceVM が Scope を所有するため）
        let a = semantic_analyze(&s).unwrap();

        let stdin_reader: Box<dyn std::io::BufRead> = Box::new(std::io::BufReader::new(
            std::io::Cursor::new(stdin_content.as_bytes().to_vec()),
        ));
        let mut vm = NospaceVM::from_scope(a)
            .expect("failed to create NospaceVM")
            .with_stdin(stdin_reader);

        let result = vm.run(1_000_000);
        match result {
            StepResult::Complete { .. } => {}
            StepResult::Suspended => panic!(
                "NospaceVM: did not complete within budget (test: {}, case: {})",
                test_name, case_name
            ),
            StepResult::Error(e) => panic!(
                "NospaceVM runtime error (test: {}, case: {}): {:?}",
                test_name, case_name, e
            ),
        }
        vm.flush();
        let actual_stdout = vm.get_stdout_string();

        assert_eq!(
            expected_stdout, actual_stdout,
            "stdout mismatch in NospaceVM test '{}', case '{}'\nExpected: {:?}\nActual: {:?}",
            test_name, case_name, expected_stdout, actual_stdout
        );
    }

    Ok(())
}

/// NospaceVM を使って runtime_error テストを実行する
///
/// 実行時エラーが発生すること（panic またはエラー返り値）を確認する。
/// 再帰インタプリタと同様に `catch_unwind` でパニックをキャッチする。
pub fn test_runtime_error_base_vm(test_name: &str) -> Result {
    let path_base = "resources/tests/fails/runtime/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = semantic_analyze(&s).ok().unwrap();

    // NospaceVM ではパニック（assert 等）もランタイムエラーとして扱う
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut vm = NospaceVM::from_scope(a).expect("failed to create NospaceVM");
        let result = vm.run(1_000_000);
        match result {
            StepResult::Error(_) => true,         // 期待通りのエラー
            StepResult::Complete { .. } => false, // エラーが発生しなかった
            StepResult::Suspended => false,       // 完了しなかった
        }
    }));

    match result {
        Err(_) => {
            // パニックが発生: ランタイムエラーとして正常
        }
        Ok(true) => {
            // StepResult::Error が返された: ランタイムエラーとして正常
        }
        Ok(false) => {
            panic!("NospaceVM: expected runtime error but completed or suspended",);
        }
    }
    Ok(())
}
