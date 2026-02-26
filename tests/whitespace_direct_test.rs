// Whitespace 直接テスト用のランナー

mod common;

#[path = "whitespace_direct_test/base.rs"]
mod base;

use base::*;

// build.rs から生成されるテスト関数は generated_ws_tests.rs にインクルードされる
include!(concat!(env!("OUT_DIR"), "/generated_ws_tests.rs"));
