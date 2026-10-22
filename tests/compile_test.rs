//! Whitespace コンパイラの統合テスト
//!
//! このファイルには compile_to_ws のテストのみを含みます。
//! 他のコンパイルテストは test-manifest.yaml で定義され、自動生成されています。

mod common;

use nospace20::{
    compile_to_ws, parse_to_tokens, parse_to_tree, semantic_analyze, WsCompileOptions,
    WsOutputFormat,
};

#[test]
fn test_compile_debug_string() {
    let source = r#"
        func: __main() {
            return: 1;
        }
    "#
    .to_string();

    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = semantic_analyze(&ast).unwrap();

    let result = compile_to_ws(
        &scope,
        &WsCompileOptions {
            output_format: WsOutputFormat::Mnemonic,
            ..Default::default()
        },
    );
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());

    let debug_str = result.unwrap();
    // デバッグ文字列にニーモニックが含まれることを確認
    assert!(debug_str.contains("push"));
    assert!(debug_str.contains("ret"));
}

/// コンパイルエラーが Vec<CodeParseError> として返ることを確認するテスト
///
/// コンパイルエラーが構造化された形式で返されることを確認する。
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
    let scope = semantic_analyze(&ast).unwrap();

    let result = compile_to_ws(&scope, &WsCompileOptions::default());
    assert!(result.is_err(), "Should fail when main function is missing");

    let error = result.unwrap_err();
    assert!(
        error.kind.to_string().contains("__main"),
        "Error message should mention 'main': {}",
        error.kind
    );
    // MainNotFound は特定の位置に紐づかないため location は None
    assert_eq!(error.location, None);
}

/// WS コンパイラのエラー（InvalidOperation）に位置情報が含まれることを確認するテスト
///
/// 意味解析を通過するが、コンパイル時にエラーとなるケース。
/// 以前は continue outside loop を使っていたが、ループの外の break / continue は
/// 意味解析で拒否するようになった（static-storage-unification ステップ 1）ので、
/// alloc 拡張なしの `__alloc` に置き換えた。
#[test]
fn test_compile_error_invalid_operation_has_location() {
    let source = "func:__main(){let: p(__alloc(1)); return:0;}";

    let tokens = parse_to_tokens(&source.to_string()).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = semantic_analyze(&ast).unwrap();

    let result = compile_to_ws(
        &scope,
        &WsCompileOptions {
            alloc_ext: false,
            ..Default::default()
        },
    );
    assert!(
        result.is_err(),
        "Should fail: __alloc without the alloc extension"
    );

    let error = result.unwrap_err();
    assert!(
        error.kind.to_string().contains("__alloc"),
        "Error message should mention '__alloc': {}",
        error.kind
    );
    assert!(
        error.location.is_some(),
        "compile error should have source location (location should be Some)"
    );
}
