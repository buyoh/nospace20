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
}

fn main() {
    // YAMLファイルが変更されたら再ビルド
    println!("cargo:rerun-if-changed=resources/tests/test-manifest.yaml");

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

        match test.test_type.as_str() {
            "success" => {
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
            "success_io" => {
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
            "syntax_error" => {
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
            _ => {
                panic!("Unknown test type: {}", test.test_type);
            }
        }
    }

    println!("Generated {} tests", test_count);
}
