//! Whitespace コンパイラの統合テスト
//!
//! このファイルには compile_to_whitespace_debug のテストのみを含みます。
//! 他のコンパイルテストは test-manifest.yaml で定義され、自動生成されています。

mod common;

use nospace20::{
    compile_to_whitespace, compile_to_whitespace_debug, parse_to_tokens, parse_to_tree,
    syntactic_analyze,
};

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

/// コンパイルエラーが Vec<CodeParseError> として返ることを確認するテスト
///
/// コンパイルエラーが構造化された形式で返されることを確認する。
/// (Phase 1: コンパイルエラーの位置情報サポート)
#[test]
fn test_compile_error_returns_code_parse_error() {
    // main 関数なしのコード → CompileError::MainNotFound
    let source = r#"
        func: foo() {
            return: 1;
        }
    "#
    .to_string();

    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();

    let result = compile_to_whitespace(&scope);
    assert!(result.is_err(), "Should fail when main function is missing");

    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should have at least one error");
    assert!(
        errors[0].message.contains("main"),
        "Error message should mention 'main': {}",
        errors[0].message
    );
    // MainNotFound は特定の位置に紐づかないため code_pointer は None
    assert_eq!(errors[0].code_pointer, None);
}

/// continue outside loop のコンパイルエラーに位置情報が含まれることを確認するテスト
///
/// セマンティクス解析を通過するが、コンパイル時にエラーとなるケース。
/// Phase 1 実装後、エラーに文レベルの位置情報が含まれることを確認する。
#[test]
fn test_compile_error_invalid_operation_has_location() {
    // continue をループ外で使用するコード
    // セマンティクス解析はパスするが、コンパイル時に continue outside loop エラーとなる
    let source = "func:main(){continue:;return:0;}";

    let tokens = parse_to_tokens(&source.to_string()).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();

    let result = compile_to_whitespace(&scope);
    assert!(result.is_err(), "Should fail: continue outside loop");

    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should have at least one error");
    assert!(
        errors[0].message.contains("continue"),
        "Error message should mention 'continue': {}",
        errors[0].message
    );
    // Phase 1: 文レベルの位置情報が含まれること
    assert!(
        errors[0].code_pointer.is_some(),
        "compile error should have source location (code_pointer should be Some)"
    );
}
