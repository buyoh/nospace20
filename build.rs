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
    #[serde(default)]
    std_ext: Option<Vec<String>>,
}

fn main() {
    // YAMLファイルが変更されたら再ビルド
    println!("cargo:rerun-if-changed=resources/tests/test-manifest.yaml");
    println!("cargo:rerun-if-changed=resources/tests_ws/test-manifest.yaml");

    // YAMLファイルを読み込み
    let manifest_path = "resources/tests/test-manifest.yaml";
    let manifest_content =
        fs::read_to_string(manifest_path).expect("Failed to read test-manifest.yaml");

    let manifest: TestManifest =
        serde_yaml::from_str(&manifest_content).expect("Failed to parse test-manifest.yaml");

    // 出力ディレクトリを取得
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_tests.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    // テストコードを生成
    let test_count = manifest.tests.len();
    for test in manifest.tests {
        let comment_line = if let Some(comment) = &test.comment {
            format!("// {}\n", comment)
        } else {
            String::new()
        };

        // exclude_targets に含まれないターゲットを有効にする（デフォルトは全ターゲット）
        let empty_targets: Vec<String> = vec![];
        let exclude_targets = test.exclude_targets.as_ref().unwrap_or(&empty_targets);
        let has_interpreter = !exclude_targets.iter().any(|t| t == "interpreter");
        let has_whitespace = !exclude_targets.iter().any(|t| t == "whitespace");
        let has_whitespace_self = !exclude_targets.iter().any(|t| t == "whitespace-self");
        
        // std_ext の有無を確認
        let has_debug_ext = test.std_ext.as_ref()
            .map(|exts| exts.iter().any(|e| e == "debug"))
            .unwrap_or(false);

        match test.test_type.as_str() {
            "success" => {
                if has_interpreter {
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

                if has_whitespace {
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

                if has_whitespace_self {
                    if has_debug_ext {
                        writeln!(
                            f,
                            r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_base_debug("{}", true)
}}
"#,
                            comment_line, test.name, test.path
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            f,
                            r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_base_debug("{}", false)
}}
"#,
                            comment_line, test.name, test.path
                        )
                        .unwrap();
                    }
                }
            }
            "success_io" => {
                if has_interpreter {
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

                if has_whitespace {
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

                if has_whitespace_self {
                    if has_debug_ext {
                        writeln!(
                            f,
                            r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_io_base_debug("{}", true)
}}
"#,
                            comment_line, test.name, test.path
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            f,
                            r#"{}#[test]
fn {}_ws_self() {{
    test_whitespace_self_io_base_debug("{}", false)
}}
"#,
                            comment_line, test.name, test.path
                        )
                        .unwrap();
                    }
                }
            }
            "syntax_error" => {
                if has_interpreter {
                    writeln!(
                        f,
                        r#"{}#[test]
fn {}() -> std::fmt::Result {{
    test_syntax_error_base("{}")
}}
"#,
                        comment_line, test.name, test.path
                    )
                    .unwrap();
                }
            }
            "compile_error" => {
                if has_interpreter {
                    writeln!(
                        f,
                        r#"{}#[test]
fn {}() -> std::fmt::Result {{
    test_compile_error_base("{}")
}}
"#,
                        comment_line, test.name, test.path
                    )
                    .unwrap();
                }
            }
            "runtime_error" => {
                if has_interpreter {
                    writeln!(
                        f,
                        r#"{}#[test]
fn {}() -> std::fmt::Result {{
    test_runtime_error_base("{}")
}}
"#,
                        comment_line, test.name, test.path
                    )
                    .unwrap();
                }
            }
            _ => {
                panic!("Unknown test type: {}", test.test_type);
            }
        }
    }

    println!("Generated {} tests", test_count);

    // Whitespace 直接テスト用の生成コード
    generate_ws_tests();
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

    let manifest: TestManifest =
        serde_yaml::from_str(&manifest_content).expect("Failed to parse tests_ws/test-manifest.yaml");

    // 出力ディレクトリを取得
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_ws_tests.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    // テストコードを生成
    let test_count = manifest.tests.len();
    for test in manifest.tests {
        let comment_line = if let Some(comment) = &test.comment {
            format!("// {}\n", comment)
        } else {
            String::new()
        };

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
            _ => {
                panic!("Unknown Whitespace test type: {}", test.test_type);
            }
        }
    }

    println!("Generated {} Whitespace tests", test_count);
}
