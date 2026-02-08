//! Whitespace コンパイラの統合テスト
//!
//! このファイルには compile_to_whitespace_debug のテストのみを含みます。
//! 他のコンパイルテストは test-manifest.yaml で定義され、自動生成されています。

mod common;

use nospace20::{compile_to_whitespace_debug, parse_to_tokens, parse_to_tree, syntactic_analyze};

#[test]
fn test_compile_debug_string() {
    let source = r#"
        func: main() {
            return: 1;
        }
    "#
    .to_string();

    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();

    let result = compile_to_whitespace_debug(&scope);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());

    let debug_str = result.unwrap();
    // デバッグ文字列にニーモニックが含まれることを確認
    assert!(debug_str.contains("push"));
    assert!(debug_str.contains("ret"));
}
