use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::common::{format_comment_line, TestManifest};

pub fn generate_ws_tests() {
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
