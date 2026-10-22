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
            "syntax_error" => write_error_test(&mut f, &test, Some("test_syntax_error_base")),
            "compile_error" => write_error_test(&mut f, &test, Some("test_compile_error_base")),
            "runtime_error" => write_error_test(&mut f, &test, None),
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

    write_ws_self_tests(f, test, &flags, "test_whitespace_self_base");
}

/// "success_io" タイプのテストコードを生成する。
fn write_success_io_tests(f: &mut fs::File, test: &TestCase) {
    let comment_line = format_comment_line(&test.comment);
    let flags = TargetFlags::from_test_case(test);

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

    write_ws_self_tests(f, test, &flags, "test_whitespace_self_io_base");
}

/// エラー系テスト（syntax_error, compile_error, runtime_error）のコードを生成する。
///
/// `base_fn` は syntax_error / compile_error の base 関数名。runtime_error は `None` で、
/// `_ws_self` のテストだけを生成する。
fn write_error_test(f: &mut fs::File, test: &TestCase, base_fn: Option<&str>) {
    let comment_line = format_comment_line(&test.comment);
    let flags = TargetFlags::from_test_case(test);

    // syntax_error / compile_error は実行を伴わないので、ターゲットによらず生成する
    if let Some(base_fn) = base_fn {
        writeln!(
            f,
            r#"{}#[test]
fn {}() {{
    {}("{}")
}}
"#,
            comment_line, test.name, base_fn, test.path
        )
        .unwrap();
        return;
    }

    // runtime_error は compile → 独自 WhitespaceVM で実行時エラーで止まることを確かめる
    if flags.has_whitespace_self {
        writeln!(
            f,
            r#"{}#[test]
fn {}_ws_self() {{
    test_runtime_error_ws_self_base("{}", {})
}}
"#,
            comment_line,
            test.name,
            test.path,
            ws_self_target_literal(&flags, WsSelfVariant::Plain)
        )
        .unwrap();
    }
}

/// `_ws_self` 系のターゲットの種類（テスト名の接尾辞と VM・最適化の設定を決める）
#[derive(Clone, Copy)]
enum WsSelfVariant {
    Plain,
    Strict,
    Randomize,
    OptAll,
}

impl WsSelfVariant {
    fn suffix(self) -> &'static str {
        match self {
            WsSelfVariant::Plain => "_ws_self",
            WsSelfVariant::Strict => "_ws_self_strict",
            WsSelfVariant::Randomize => "_ws_self_randomize",
            WsSelfVariant::OptAll => "_ws_self_opt_all",
        }
    }
}

/// 生成するテストコードに埋め込む `WsSelfTarget` の構造体リテラル
/// （`tests/code_test/whitespace_self_base.rs` で定義）
fn ws_self_target_literal(flags: &TargetFlags, variant: WsSelfVariant) -> String {
    format!(
        "WsSelfTarget {{ debug_ext: {}, alloc_ext: {}, strict_heap: {}, randomize_heap: {}, opt_all: {} }}",
        flags.has_debug_ext,
        flags.has_alloc_ext,
        matches!(variant, WsSelfVariant::Strict),
        matches!(variant, WsSelfVariant::Randomize),
        matches!(variant, WsSelfVariant::OptAll),
    )
}

/// success / success_io の `_ws_self` 系のテスト（compile → 独自 WhitespaceVM）を生成する。
///
/// `base_fn` は `fn(test_name: &str, target: WsSelfTarget)` の形の base 関数名。
fn write_ws_self_tests(f: &mut fs::File, test: &TestCase, flags: &TargetFlags, base_fn: &str) {
    let comment_line = format_comment_line(&test.comment);
    let variants = [
        (flags.has_whitespace_self, WsSelfVariant::Plain),
        (flags.has_whitespace_self_strict, WsSelfVariant::Strict),
        (flags.has_whitespace_self_opt_all, WsSelfVariant::OptAll),
        (
            flags.has_whitespace_self_randomize,
            WsSelfVariant::Randomize,
        ),
    ];
    for (enabled, variant) in variants {
        if !enabled {
            continue;
        }
        writeln!(
            f,
            r#"{}#[test]
fn {}{}() {{
    {}("{}", {})
}}
"#,
            comment_line,
            test.name,
            variant.suffix(),
            base_fn,
            test.path,
            ws_self_target_literal(flags, variant)
        )
        .unwrap();
    }
}
