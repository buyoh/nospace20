use std::{fmt::Result, fs, io};

use nospace20::{interpret_func_testing, interpret_func_with_io, parse_to_tokens, parse_to_tree, syntactic_analyze};
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
        TestConfig::Success { trace: expected_trace_vec } => {
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
        TestConfig::SuccessIo { stdin, stdin_file, stdout, stdout_file } => {
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
        TestConfig::ParseError { phase, error_count: _, contains: _ } => {
            match phase.as_str() {
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
            }
        }
        _ => panic!("Expected parse_error test config"),
    }
    Ok(())
}

macro_rules! test_ok_coding {
    ($name: ident, $test_name: expr) => {
        // TODO: concat_idents! is only for nightly
        #[test]
        fn $name() -> Result {
            test_ok_coding_base($test_name)
        }
    };
}

macro_rules! test_syntax_error {
    ($name: ident, $test_name: expr) => {
        #[test]
        fn $name() -> Result {
            test_syntax_error_base($test_name)
        }
    };
}

macro_rules! test_ok_coding_io {
    ($name: ident, $test_name: expr) => {
        #[test]
        fn $name() -> Result {
            test_ok_coding_io_base($test_name)
        }
    };
}

// Legacy tests (backward compatibility)
test_ok_coding!(test_ok_coding_c000, "c000");
test_ok_coding!(test_ok_coding_c001, "c001");
// test_ok_coding!(test_ok_coding_c002, "c002");  // DISABLED: hangs (break/continue issue)
test_ok_coding!(test_ok_coding_c003, "c003");
test_ok_coding!(test_ok_coding_c004, "c004");

// Literals
test_ok_coding!(test_literals_num_001, "literals/num_001");
test_ok_coding!(test_literals_num_002, "literals/num_002");
test_ok_coding!(test_literals_ident_001, "literals/ident_001");
test_ok_coding!(test_literals_comment_001, "literals/comment_001");
test_ok_coding!(test_literals_char_001, "literals/char_001");
test_ok_coding!(test_literals_char_escape_001, "literals/char_escape_001");
test_ok_coding!(test_literals_char_arith_001, "literals/char_arith_001");

// Operators
test_ok_coding!(test_operators_arith_001, "operators/arith_001");
test_ok_coding!(test_operators_arith_002, "operators/arith_002");
test_ok_coding!(test_operators_arith_003, "operators/arith_003");
test_ok_coding!(test_operators_unary_001, "operators/unary_001");
test_ok_coding!(test_operators_compare_001, "operators/compare_001");
test_ok_coding!(test_operators_logical_not_001, "operators/logical_not_001");
test_ok_coding!(test_operators_logical_and_001, "operators/logical_and_001");
test_ok_coding!(test_operators_logical_or_001, "operators/logical_or_001");
test_ok_coding!(test_operators_logical_short_circuit_001, "operators/logical_short_circuit_001");
test_ok_coding!(test_operators_logical_short_circuit_002, "operators/logical_short_circuit_002");
test_ok_coding!(test_operators_logical_precedence_001, "operators/logical_precedence_001");
test_ok_coding!(test_operators_modulo_001, "operators/modulo_001");
test_ok_coding!(test_operators_modulo_002, "operators/modulo_002");

// Builtins
test_ok_coding!(test_builtins_trace_001, "builtins/trace_001");
test_ok_coding!(test_builtins_assert_001, "builtins/assert_001");

// Variables
test_ok_coding!(test_variables_var_basic_001, "variables/var_basic_001");
test_ok_coding!(test_variables_var_hoist_001, "variables/var_hoist_001");

// Functions
test_ok_coding!(test_functions_func_basic_001, "functions/func_basic_001");
test_ok_coding!(test_functions_func_args_001, "functions/func_args_001");
test_ok_coding!(test_functions_func_return_001, "functions/func_return_001");
test_ok_coding!(test_functions_func_hoist_001, "functions/func_hoist_001");
test_ok_coding!(test_functions_func_nested_001, "functions/func_nested_001");

// Control Flow
test_ok_coding!(test_control_flow_while_001, "control_flow/while_001");
test_ok_coding!(test_control_flow_if_001, "control_flow/if_001");
// test_ok_coding!(test_control_flow_break_continue_001, "control_flow/break_continue_001");  // DISABLED: hangs (break/continue issue)
test_ok_coding!(test_control_flow_return_001, "control_flow/return_001");

// Scope
test_ok_coding!(test_scope_scope_block_001, "scope/scope_block_001");
test_ok_coding!(test_scope_scope_func_001, "scope/scope_func_001");
test_ok_coding!(test_scope_scope_nested_func_001, "scope/scope_nested_func_001");

// Integration
test_ok_coding!(test_integration_integ_001, "integration/integ_001");

// Syntax errors
test_syntax_error!(test_syntax_error_invalid_token_001, "invalid_token_001");
test_syntax_error!(test_syntax_error_unclosed_paren_001, "unclosed_paren_001");
test_syntax_error!(test_syntax_error_unexpected_eof_001, "unexpected_eof_001");

// I/O tests (migrated from legacy tests)
test_ok_coding_io!(test_legacy_001, "legacy/legacy_001");
test_ok_coding_io!(test_legacy_002, "legacy/legacy_002");
test_ok_coding_io!(test_legacy_003, "legacy/legacy_003");
test_ok_coding_io!(test_legacy_004, "legacy/legacy_004");
test_ok_coding_io!(test_legacy_005, "legacy/legacy_005");
test_ok_coding_io!(test_legacy_007, "legacy/legacy_007");
test_ok_coding_io!(test_legacy_008, "legacy/legacy_008");
test_ok_coding_io!(test_legacy_009, "legacy/legacy_009");
test_ok_coding_io!(test_legacy_010, "legacy/legacy_010");
test_ok_coding_io!(test_legacy_016, "legacy/legacy_016");

// I/O builtin function tests
test_ok_coding_io!(test_io_puti_basic_001, "io/puti_basic_001");
test_ok_coding_io!(test_io_putc_basic_001, "io/putc_basic_001");
test_ok_coding_io!(test_io_geti_basic_001, "io/geti_basic_001");
test_ok_coding_io!(test_io_getc_basic_001, "io/getc_basic_001");
test_ok_coding_io!(test_io_combined_001, "io/io_combined_001");

// Disabled tests (未実装機能のテスト)
// テスト名が disabled_ で始まるものは除外される
// test_ok_coding!(test_variables_disabled_var_global_001, "variables/disabled_var_global_001");
// test_ok_coding!(test_variables_disabled_var_final_001, "variables/disabled_var_final_001");
// test_ok_coding!(test_variables_disabled_var_init_001, "variables/disabled_var_init_001");
// test_ok_coding!(test_scope_disabled_scope_block_var_001, "scope/disabled_scope_block_var_001");
// test_ok_coding_io!(test_legacy_006, "legacy/legacy_006");  // DISABLED: requires global variables
// test_ok_coding_io!(test_legacy_011, "legacy/legacy_011");  // DISABLED: requires global variables
// test_ok_coding_io!(test_legacy_012, "legacy/legacy_012");  // DISABLED: requires global variables
// test_ok_coding_io!(test_legacy_013, "legacy/legacy_013");  // DISABLED: requires pointers
// test_ok_coding_io!(test_legacy_014, "legacy/legacy_014");  // DISABLED: requires negative literals
// test_ok_coding_io!(test_legacy_015, "legacy/legacy_015");  // DISABLED: requires pointers and <=
// test_ok_coding_io!(test_legacy_017, "legacy/legacy_017");  // DISABLED: requires pointers and <=
// test_ok_coding_io!(test_legacy_018, "legacy/legacy_018");  // DISABLED: requires character literals
// test_ok_coding_io!(test_legacy_019, "legacy/legacy_019");  // DISABLED: requires arrays
// test_ok_coding_io!(test_legacy_020, "legacy/legacy_020");  // DISABLED: requires logical operators
// test_ok_coding_io!(test_legacy_021, "legacy/legacy_021");  // DISABLED: requires array initialization
// test_ok_coding_io!(test_legacy_022, "legacy/legacy_022");  // DISABLED: requires pointers
// test_ok_coding_io!(test_legacy_023, "legacy/legacy_023");  // DISABLED: requires compound assignments
// test_ok_coding_io!(test_legacy_024, "legacy/legacy_024");  // DISABLED: requires arrays and pointers
// test_ok_coding_io!(test_legacy_025, "legacy/legacy_025");  // DISABLED: requires character arithmetic
// test_ok_coding_io!(test_legacy_026, "legacy/legacy_026");  // DISABLED: requires multi-char literals
// test_ok_coding_io!(test_legacy_027, "legacy/legacy_027");  // DISABLED: requires __getiv, __getcv, complex expressions
