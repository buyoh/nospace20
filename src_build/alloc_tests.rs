use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::common::{format_comment_line, TestManifest};

pub fn generate_alloc_tests() {
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
