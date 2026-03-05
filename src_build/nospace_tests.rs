use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::common::{format_comment_line, TargetFlags, TestCase, TestManifest};

/// nospace テストコードを生成する。
pub fn generate_nospace_tests() {
    let manifest_path = "resources/tests/test-manifest.yaml";
    let manifest_content =
        fs::read_to_string(manifest_path).expect("Failed to read test-manifest.yaml");

    let manifest: TestManifest =
        serde_yaml::from_str(&manifest_content).expect("Failed to parse test-manifest.yaml");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_tests.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    let test_count = manifest.tests.len();
    for test in manifest.tests {
        match test.test_type.as_str() {
            "success" => write_success_tests(&mut f, &test),
            "success_io" => write_success_io_tests(&mut f, &test),
            "syntax_error" => write_error_test(&mut f, &test, "test_syntax_error_base"),
            "compile_error" => write_error_test(&mut f, &test, "test_compile_error_base"),
            "runtime_error" => write_error_test(&mut f, &test, "test_runtime_error_base"),
            _ => {
                panic!("Unknown test type: {}", test.test_type);
            }
        }
    }

    println!("Generated {} tests", test_count);
}

/// "success" タイプのテストコードを生成する。
fn write_success_tests(f: &mut fs::File, test: &TestCase) {
    let comment_line = format_comment_line(&test.comment);
    let flags = TargetFlags::from_test_case(test);

    if flags.has_interpreter {
        writeln!(
            f,
            r#"{}#[test]
fn {}() -> std::fmt::Result {{
    test_ok_coding_base("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_nospace_vm {
        writeln!(
            f,
            r#"{}#[test]
fn {}_vm() -> std::fmt::Result {{
    test_ok_coding_base_vm("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_whitespace {
        writeln!(
            f,
            r#"{}#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn {}_ws() {{
    test_whitespace_base("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_whitespace_self {
        if flags.has_alloc_ext {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_base_alloc("{}", {}, true)
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        } else {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_base_debug("{}", {})
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        }
    }

    if flags.has_whitespace_self_strict {
        if flags.has_alloc_ext {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_strict() {{
    test_whitespace_self_base_alloc("{}", {}, true)
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        } else {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_strict() {{
    test_whitespace_self_base_strict("{}", {})
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        }
    }

    if flags.has_interpreter_opt_all {
        writeln!(
            f,
            r#"{}#[test]
fn {}_opt_all() -> std::fmt::Result {{
    test_ok_coding_base_opt_all("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_interpreter_randomize {
        writeln!(
            f,
            r#"{}#[test]
fn {}_randomize() -> std::fmt::Result {{
    test_ok_coding_base_randomize("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_whitespace_self_opt_all {
        writeln!(
            f,
            r#"{}#[test]
fn {}_ws_self_opt_all() {{
    test_whitespace_self_base_opt_all("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_whitespace_self_randomize {
        if flags.has_alloc_ext {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_randomize() {{
    test_whitespace_self_base_alloc("{}", {}, true)
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        } else {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_randomize() {{
    test_whitespace_self_base_randomize("{}", {})
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        }
    }
}

/// "success_io" タイプのテストコードを生成する。
fn write_success_io_tests(f: &mut fs::File, test: &TestCase) {
    let comment_line = format_comment_line(&test.comment);
    let flags = TargetFlags::from_test_case(test);

    if flags.has_interpreter {
        writeln!(
            f,
            r#"{}#[test]
fn {}() -> std::fmt::Result {{
    test_ok_coding_io_base("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_nospace_vm {
        writeln!(
            f,
            r#"{}#[test]
fn {}_vm() -> std::fmt::Result {{
    test_ok_coding_io_base_vm("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_interpreter_opt_all {
        writeln!(
            f,
            r#"{}#[test]
fn {}_opt_all() -> std::fmt::Result {{
    test_ok_coding_io_base_opt_all("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_whitespace {
        writeln!(
            f,
            r#"{}#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn {}_ws() {{
    test_whitespace_io_base("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_whitespace_self {
        if flags.has_alloc_ext {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_io_base_alloc("{}", {}, true)
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        } else {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_io_base_debug("{}", {})
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        }
    }

    if flags.has_whitespace_self_strict {
        if flags.has_alloc_ext {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_strict() {{
    test_whitespace_self_io_base_alloc("{}", {}, true)
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        } else {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_strict() {{
    test_whitespace_self_io_base_strict("{}", {})
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        }
    }

    if flags.has_whitespace_self_opt_all {
        writeln!(
            f,
            r#"{}#[test]
fn {}_ws_self_opt_all() {{
    test_whitespace_self_io_base_opt_all("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }

    if flags.has_whitespace_self_randomize {
        if flags.has_alloc_ext {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_randomize() {{
    test_whitespace_self_io_base_alloc("{}", {}, true)
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        } else {
            writeln!(
                f,
                r#"{}#[test]
fn {}_ws_self_randomize() {{
    test_whitespace_self_io_base_randomize("{}", {})
}}
"#,
                comment_line, test.name, test.path, flags.has_debug_ext
            )
            .unwrap();
        }
    }
}

/// エラー系テスト（syntax_error, compile_error, runtime_error）のコードを生成する。
fn write_error_test(f: &mut fs::File, test: &TestCase, base_fn: &str) {
    let comment_line = format_comment_line(&test.comment);
    let flags = TargetFlags::from_test_case(test);

    if flags.has_interpreter {
        writeln!(
            f,
            r#"{}#[test]
fn {}() -> std::fmt::Result {{
    {}("{}")
}}
"#,
            comment_line, test.name, base_fn, test.path
        )
        .unwrap();
    }

    // runtime_error のみ NospaceVM テストも生成
    if base_fn == "test_runtime_error_base" && flags.has_nospace_vm {
        writeln!(
            f,
            r#"{}#[test]
fn {}_vm() -> std::fmt::Result {{
    test_runtime_error_base_vm("{}")
}}
"#,
            comment_line, test.name, test.path
        )
        .unwrap();
    }
}
