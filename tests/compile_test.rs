//! Whitespace コンパイラの統合テスト

use nospace20::{parse_to_tokens, parse_to_tree, syntactic_analyze, compile_to_whitespace, compile_to_whitespace_debug};

#[test]
fn test_compile_empty_main() {
    let source = r#"
        func: main() {
            return: 0;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    
    let result = compile_to_whitespace(&scope);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
    
    let ws_code = result.unwrap();
    // Whitespace コードが生成されていることを確認
    assert!(!ws_code.is_empty());
    // 使用されている文字が空白のみであることを確認
    assert!(ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'));
}

#[test]
fn test_compile_return_42() {
    let source = r#"
        func: main() {
            return: 42;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    
    let result = compile_to_whitespace(&scope);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
}

#[test]
fn test_compile_debug_string() {
    let source = r#"
        func: main() {
            return: 1;
        }
    "#.to_string();
    
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

#[test]
fn test_compile_arithmetic() {
    let source = r#"
        func: main() {
            return: 1 + 2 * 3;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    
    let result = compile_to_whitespace(&scope);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
}

#[test]
fn test_compile_comparison() {
    let source = r#"
        func: main() {
            return: 5 < 10;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    
    let result = compile_to_whitespace(&scope);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
}

#[test]
fn test_compile_logical() {
    let source = r#"
        func: main() {
            return: 1 && 0;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    
    let result = compile_to_whitespace(&scope);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
}

#[test]
fn test_compile_variable() {
    let source = r#"
        func: main() {
            let: x;
            x = 10;
            return: x;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    
    let result = compile_to_whitespace(&scope);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
}

#[test]
fn test_compile_no_main() {
    let source = r#"
        func: foo() {
            return: 1;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast).unwrap();
    
    let result = compile_to_whitespace(&scope);
    // main 関数がないのでエラーになるはず
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("main"));
}
