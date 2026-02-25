use serde::Deserialize;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct TestManifest {
    tests: Vec<TestCase>,
}

#[derive(Debug, Deserialize)]
struct TestCase {
    name: String,
    #[serde(rename = "type")]
    test_type: String,
    path: String,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    exclude_targets: Option<Vec<String>>,
    #[allow(dead_code)]
    #[serde(default)]
    std_ext: Option<Vec<String>>,
    #[serde(default)]
    exclude_std_ext: Option<Vec<String>>,
}

fn main() {
    // YAMLファイルが変更されたら再ビルド
    println!("cargo:rerun-if-changed=resources/tests/test-manifest.yaml");
    println!("cargo:rerun-if-changed=resources/tests_ws/test-manifest.yaml");

    generate_nospace_tests();
    generate_ws_tests();
    generate_alloc_tests();
}

/// コメント行文字列を生成する。コメントがある場合は `// <comment>\n` を返す。
fn format_comment_line(comment: &Option<String>) -> String {
    if let Some(comment) = comment {
        format!("// {}\n", comment)
    } else {
        String::new()
    }
}

/// テストケースの exclude_targets から各ターゲットの有効/無効を判定する。
struct TargetFlags {
    has_interpreter: bool,
    has_interpreter_randomize: bool,
    has_whitespace: bool,
    has_whitespace_self: bool,
    has_whitespace_self_strict: bool,
    has_whitespace_self_randomize: bool,
    has_debug_ext: bool,
}

impl TargetFlags {
    fn from_test_case(test: &TestCase) -> Self {
        let empty_targets: Vec<String> = vec![];
        let exclude_targets = test.exclude_targets.as_ref().unwrap_or(&empty_targets);
        let has_debug_ext = test
            .exclude_std_ext
            .as_ref()
            .map(|exts| !exts.iter().any(|e| e == "debug"))
            .unwrap_or(true);
        Self {
            has_interpreter: !exclude_targets.iter().any(|t| t == "interpreter"),
            has_interpreter_randomize: !exclude_targets
                .iter()
                .any(|t| t == "interpreter-randomize"),
            has_whitespace: !exclude_targets.iter().any(|t| t == "whitespace"),
            has_whitespace_self: !exclude_targets.iter().any(|t| t == "whitespace-self"),
            has_whitespace_self_strict: !exclude_targets
                .iter()
                .any(|t| t == "whitespace-self-strict"),
            has_whitespace_self_randomize: !exclude_targets
                .iter()
                .any(|t| t == "whitespace-self-randomize"),
            has_debug_ext,
        }
    }
}

/// nospace テストコードを生成する。
fn generate_nospace_tests() {
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

    if flags.has_whitespace_self_strict {
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

    if flags.has_whitespace_self_randomize {
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

    if flags.has_whitespace_self_strict {
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

    if flags.has_whitespace_self_randomize {
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
}

fn generate_ws_tests() {
    // test-manifest.yaml を読み込み
    let manifest_path = "resources/tests_ws/test-manifest.yaml";

    // ファイルが存在しない場合はスキップ
    if !Path::new(manifest_path).exists() {
        println!("Skipping Whitespace tests generation (manifest not found)");
        return;
    }

    let manifest_content =
        fs::read_to_string(manifest_path).expect("Failed to read tests_ws/test-manifest.yaml");

    let manifest: TestManifest = serde_yaml::from_str(&manifest_content)
        .expect("Failed to parse tests_ws/test-manifest.yaml");

    // 出力ディレクトリを取得
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_ws_tests.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    // テストコードを生成
    let test_count = manifest.tests.len();
    for test in manifest.tests {
        let comment_line = format_comment_line(&test.comment);

        match test.test_type.as_str() {
            "ws_io" => {
                // 通常の WhitespaceVM テスト
                writeln!(
                    f,
                    r#"{}#[test]
fn {}() {{
    test_ws_io_base("{}")
}}
"#,
                    comment_line, test.name, test.path
                )
                .unwrap();

                // wsc クロスバリデーションテスト
                writeln!(
                    f,
                    r#"#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn {}_wsc() {{
    test_ws_io_wsc_base("{}")
}}
"#,
                    test.name, test.path
                )
                .unwrap();
            }
            "ws_runtime_error" => {
                // 通常の WhitespaceVM テスト
                writeln!(
                    f,
                    r#"{}#[test]
fn {}() {{
    test_ws_runtime_error_base("{}")
}}
"#,
                    comment_line, test.name, test.path
                )
                .unwrap();

                // wsc クロスバリデーションテスト
                writeln!(
                    f,
                    r#"#[test]
#[ignore = "requires wsc (./tools/setup-wsc.sh)"]
fn {}_wsc() {{
    test_ws_runtime_error_wsc_base("{}")
}}
"#,
                    test.name, test.path
                )
                .unwrap();
            }
            "ws_parse_error" => {
                // パースエラーテスト
                writeln!(
                    f,
                    r#"{}#[test]
fn {}() {{
    test_ws_parse_error_base("{}")
}}
"#,
                    comment_line, test.name, test.path
                )
                .unwrap();
            }
            _ => {
                panic!("Unknown Whitespace test type: {}", test.test_type);
            }
        }
    }

    println!("Generated {} Whitespace tests", test_count);
}

fn generate_alloc_tests() {
    let manifest_path = "resources/tests_alloc/test-manifest.yaml";

    println!("cargo:rerun-if-changed={}", manifest_path);

    // ファイルが存在しない場合はスキップ
    if !Path::new(manifest_path).exists() {
        println!("Skipping alloc tests generation (manifest not found)");
        return;
    }

    let manifest_content =
        fs::read_to_string(manifest_path).expect("Failed to read tests_alloc/test-manifest.yaml");

    let manifest: TestManifest = serde_yaml::from_str(&manifest_content)
        .expect("Failed to parse tests_alloc/test-manifest.yaml");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_alloc_tests.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    let test_count = manifest.tests.len();
    for test in manifest.tests {
        let comment_line = format_comment_line(&test.comment);

        match test.test_type.as_str() {
            "alloc_io" | "alloc_runtime_error" => {
                writeln!(
                    f,
                    r#"{}#[test]
fn {}() {{
    run_alloc_test("{}")
}}
"#,
                    comment_line, test.name, test.path
                )
                .unwrap();
            }
            _ => {
                panic!("Unknown alloc test type: {}", test.test_type);
            }
        }
    }

    println!("Generated {} alloc tests", test_count);
}
