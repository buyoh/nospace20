//! メモリアロケータ分離テスト
//!
//! JSON テスト仕様をパースし、ミニコンパイラで WS 命令列に変換、
//! WhitespaceVM 上で実行して期待出力を検証する。
//!
//! nospace パイプライン（token_parser, tree_parser, semantic_analyzer, interpreter）に
//! 一切依存せず、alloc_runtime + WhitespaceVM のみで完結する。

#[path = "alloc_test/test_spec.rs"]
mod test_spec;
#[path = "alloc_test/mini_compiler.rs"]
mod mini_compiler;
#[path = "alloc_test/runner.rs"]
mod runner;

use runner::run_alloc_test;

// === build.rs で生成されたテスト関数をインクルード ===
include!(concat!(env!("OUT_DIR"), "/generated_alloc_tests.rs"));
