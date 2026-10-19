//! ネスト深度上限（`MAX_NESTING_DEPTH`）のテスト

use super::*;

fn parse_src(src: &str) -> Result<Vec<LocatedStatement>, Vec<CodeParseError>> {
    let tokens = crate::token_parser::parse_to_tokens(&src.to_string()).expect("tokenize failed");
    parse_to_tree(&tokens)
}

fn assert_ok(src: &str) {
    if let Err(errs) = parse_src(src) {
        panic!("expected success, got {:?}", errs);
    }
}

fn assert_too_deep(src: &str) {
    match parse_src(src) {
        Ok(_) => panic!("expected nesting error"),
        Err(errs) => assert!(
            errs.iter().any(|e| e.message == NESTING_TOO_DEEP_MESSAGE),
            "expected nesting error, got {:?}",
            errs
        ),
    }
}

fn main_returning(expr: &str) -> String {
    format!("func: __main() {{ return: {}; }}", expr)
}

#[test]
fn test_message_mentions_limit() {
    assert!(NESTING_TOO_DEEP_MESSAGE.contains(&MAX_NESTING_DEPTH.to_string()));
}

#[test]
fn test_parens() {
    let n = MAX_NESTING_DEPTH - 8;
    assert_ok(&main_returning(&format!(
        "{}1{}",
        "(".repeat(n),
        ")".repeat(n)
    )));
    let n = MAX_NESTING_DEPTH * 4;
    assert_too_deep(&main_returning(&format!(
        "{}1{}",
        "(".repeat(n),
        ")".repeat(n)
    )));
}

#[test]
fn test_unary() {
    assert_ok(&main_returning(&format!(
        "{}1",
        "-".repeat(MAX_NESTING_DEPTH - 8)
    )));
    assert_too_deep(&main_returning(&format!(
        "{}1",
        "-".repeat(MAX_NESTING_DEPTH * 4)
    )));
}

#[test]
fn test_binary_chain() {
    // 左結合の連鎖は左深の構文木になるため、パーサの再帰が浅くても深さとして数える
    let chain = |n: usize| vec!["1"; n].join(" + ");
    assert_ok(&main_returning(&chain(MAX_NESTING_DEPTH - 8)));
    assert_too_deep(&main_returning(&chain(MAX_NESTING_DEPTH * 4)));
    let mul_chain = vec!["2"; MAX_NESTING_DEPTH * 4].join(" * ");
    assert_too_deep(&main_returning(&mul_chain));
}

#[test]
fn test_nested_operand_of_chain() {
    // 各段の括弧が「長い連鎖の左端のオペランド」になっている形。
    // 括弧の段数・連鎖の長さは個別には小さいが、構文木の深さは積になる
    let mut e = "1".to_string();
    for _ in 0..20 {
        e = format!("({} + {})", e, vec!["1"; 20].join(" + "));
    }
    assert_too_deep(&main_returning(&e));
}

#[test]
fn test_assign_chain() {
    let src = |n: usize| format!("func: __main() {{ let: a; {}1; }}", "a = ".repeat(n));
    assert_ok(&src(MAX_NESTING_DEPTH - 8));
    assert_too_deep(&src(MAX_NESTING_DEPTH * 4));
}

#[test]
fn test_blocks() {
    let src = |n: usize| {
        format!(
            "func: __main() {{ {}1;{} }}",
            "{ ".repeat(n),
            " };".repeat(n)
        )
    };
    assert_ok(&src(MAX_NESTING_DEPTH / 2 - 8));
    assert_too_deep(&src(MAX_NESTING_DEPTH * 4));
}

#[test]
fn test_while() {
    let src = |n: usize| {
        format!(
            "func: __main() {{ {}1;{} }}",
            "while: 0 { ".repeat(n),
            " };".repeat(n)
        )
    };
    assert_ok(&src(MAX_NESTING_DEPTH / 2 - 8));
    assert_too_deep(&src(MAX_NESTING_DEPTH * 4));
}

#[test]
fn test_else_if_chain() {
    let src = |n: usize| {
        format!(
            "func: __main() {{ if: 0 {{ 0; }} {}else: {{ 1; }}; }}",
            "else: if: 0 { 0; } ".repeat(n)
        )
    };
    assert_ok(&src(MAX_NESTING_DEPTH / 2 - 8));
    assert_too_deep(&src(MAX_NESTING_DEPTH * 4));
}

#[test]
fn test_call_args() {
    let n = MAX_NESTING_DEPTH * 4;
    assert_too_deep(&main_returning(&format!(
        "{}1{}",
        "f(".repeat(n),
        ")".repeat(n)
    )));
}

#[test]
fn test_postfix_chains() {
    let n = MAX_NESTING_DEPTH * 8;
    // 後置演算子の連鎖もループで左深の構文木を作る
    assert_too_deep(&main_returning(&format!("a{}", "[0]".repeat(n))));
    assert_too_deep(&main_returning(&format!("(a){}", "[0]".repeat(n))));
    assert_too_deep(&main_returning(&format!("a{}", ".x".repeat(n))));
    assert_too_deep(&main_returning(&format!("a{}", "@int".repeat(n))));
    assert_ok(&main_returning(&format!(
        "a{}",
        ".x".repeat(MAX_NESTING_DEPTH - 8)
    )));
}

#[test]
fn test_type_spec_depth() {
    let n = MAX_NESTING_DEPTH * 8;
    assert_too_deep(&main_returning(&format!("a@{}int", "&".repeat(n))));
    assert_too_deep(&main_returning(&format!("a@int{}", "[1]".repeat(n))));
    assert_too_deep(&format!(
        "func: __main() {{ let: a@{}int; }}",
        "&".repeat(n)
    ));
}

#[test]
fn test_repeat_nesting() {
    // repeat: は for: とブロック式に脱糖され、1 段で構文木が深くなる
    let src = |n: usize| {
        let mut body = "1;".to_string();
        for i in 0..n {
            body = format!("repeat: i{}(0), 1, {{ {} }};", i, body);
        }
        format!("func: __main() {{ {} }}", body)
    };
    assert_ok(&src(MAX_NESTING_DEPTH / 4 - 4));
    assert_too_deep(&src(MAX_NESTING_DEPTH));
}

#[test]
fn test_malformed_nested_index_terminates() {
    // エラー回復中に後置演算子の連鎖で深い構文木を作らないこと
    assert_too_deep(&main_returning(&"a[".repeat(MAX_NESTING_DEPTH * 8)));
}
