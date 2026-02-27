#[path = "src_build/alloc_tests.rs"]
mod alloc_tests;
#[path = "src_build/common.rs"]
mod common;
#[path = "src_build/nospace_tests.rs"]
mod nospace_tests;
#[path = "src_build/ws_tests.rs"]
mod ws_tests;

fn main() {
    // YAMLファイルが変更されたら再ビルド
    println!("cargo:rerun-if-changed=resources/tests/test-manifest.yaml");
    println!("cargo:rerun-if-changed=resources/tests_ws/test-manifest.yaml");

    nospace_tests::generate_nospace_tests();
    ws_tests::generate_ws_tests();
    alloc_tests::generate_alloc_tests();
}
