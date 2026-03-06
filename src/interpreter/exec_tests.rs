use super::*;
use crate::parse_to_tokens;
use crate::parse_to_tree;
use crate::semantic_analyzer::analyze;
use crate::EnvironmentConfig;
use std::io::Cursor;

fn create_test_env() -> Environment {
    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(Vec::<u8>::new())));
    let stdout_buf: Box<dyn std::io::Write> = Box::new(Vec::<u8>::new());
    Environment::new_with_config(stdin_cursor, stdout_buf, EnvironmentConfig::new())
}

fn create_test_env_with_stdout_capture() -> (Environment, std::rc::Rc<std::cell::RefCell<Vec<u8>>>)
{
    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(Vec::<u8>::new())));
    let stdout_buf = std::rc::Rc::new(std::cell::RefCell::new(Vec::<u8>::new()));
    let writer = crate::base::shared_writer::SharedWriter(std::rc::Rc::clone(&stdout_buf));
    let stdout: Box<dyn std::io::Write> = Box::new(writer);
    (
        Environment::new_with_config(stdin_cursor, stdout, EnvironmentConfig::new()),
        stdout_buf,
    )
}

fn parse_and_analyze(code: &str) -> Scope {
    let code_string = code.to_string();
    let tokens = parse_to_tokens(&code_string).expect("Failed to parse tokens");
    let tree = parse_to_tree(&tokens).expect("Failed to parse tree");
    analyze(&tree).expect("Failed to analyze")
}

#[test]
fn test_resolve_address_local_variables() {
    let code = r#"
func: __main() {
    let: x; let: p;
    x = 42;
    p = 0;
    return: 0;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    // Phase 2+3: global_base_addr はアロケータ経由で設定（interpret_global で初期化）
    // ここでは直接 new_func を呼び出すためアロケータは空の状態

    let func = scope.get_function("__main").unwrap();
    let local_env = LocalEnvironment::new_func(&mut env, &scope, &func, &vec![]);

    // main 関数のローカル変数 x (local_index=0), p (local_index=1)
    let id_x = IdentifierRef {
        is_global: false,
        scope_depth: 0,
        local_index: 0,
        owning_func_index: None,
    };
    let addr_x = local_env.resolve_address(&id_x);

    let id_p = IdentifierRef {
        is_global: false,
        scope_depth: 0,
        local_index: 1,
        owning_func_index: None,
    };
    let addr_p = local_env.resolve_address(&id_p);
    // Phase 2+3: アロケータはアドレス 1 から割り当てるため絶対値は不定
    // x と p は連続するローカルスロットなので差が 1 であることだけ確認する
    assert_eq!(addr_p - addr_x, 1, "p should be 1 slot after x");
}

#[test]
fn test_get_set_by_address() {
    let code = r#"
func: __main() {
    let: x; let: p;
    return: 0;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    // Phase 2+3: global_base_addr はアロケータ経由で設定（interpret_global で初期化）
    // ここでは直接 new_func を呼び出すためアロケータは空の状態

    let func = scope.get_function("__main").unwrap();
    let mut local_env = LocalEnvironment::new_func(&mut env, &scope, &func, &vec![]);

    // Phase 2+3: アロケータ経由のため固定アドレスではなく resolve_address を使用
    let id_x = IdentifierRef {
        is_global: false,
        scope_depth: 0,
        local_index: 0,
        owning_func_index: None,
    };
    let id_p = IdentifierRef {
        is_global: false,
        scope_depth: 0,
        local_index: 1,
        owning_func_index: None,
    };
    let addr_x = local_env.resolve_address(&id_x);
    let addr_p = local_env.resolve_address(&id_p);

    // addr_x に値を設定
    local_env.set_by_address(addr_x, 42);
    let val = local_env.get_by_address(addr_x);
    assert_eq!(
        val, 42,
        "get_by_address should return the value set by set_by_address"
    );

    // addr_p に値を設定
    local_env.set_by_address(addr_p, 99);
    let val = local_env.get_by_address(addr_p);
    assert_eq!(val, 99, "get_by_address should return 99");
}

#[test]
fn test_ref_and_deref_integration() {
    let code = r#"
func: __main() {
    let: x; let: p;
    x = 42;
    p = &x;
    return: *p;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();

    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(
        result,
        Ok(Some(42)),
        "should return the value of *p which is 42"
    );
}

#[test]
fn test_deref_assign_integration() {
    let code = r#"
func: __main() {
    let: x; let: p;
    x = 10;
    p = &x;
    *p = 20;
    return: x;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();

    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(
        result,
        Ok(Some(20)),
        "x should be modified to 20 via *p = 20"
    );
}

// --- T1: 組み込み関数テスト ---

fn create_test_env_with_stdin(
    stdin_data: &str,
) -> (Environment, std::rc::Rc<std::cell::RefCell<Vec<u8>>>) {
    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(
        stdin_data.as_bytes().to_vec(),
    )));
    let stdout_buf = std::rc::Rc::new(std::cell::RefCell::new(Vec::<u8>::new()));
    let writer = crate::base::shared_writer::SharedWriter(std::rc::Rc::clone(&stdout_buf));
    let stdout: Box<dyn std::io::Write> = Box::new(writer);
    (
        Environment::new_with_config(stdin_cursor, stdout, EnvironmentConfig::new()),
        stdout_buf,
    )
}

fn get_stdout_from_capture(capture: &std::rc::Rc<std::cell::RefCell<Vec<u8>>>) -> String {
    String::from_utf8(capture.borrow().clone()).unwrap()
}

#[test]
fn test_builtin_trace() {
    let code = r#"
func: __main() {
    __trace(1);
    __trace(1);
    __trace(2);
    return: 0;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    crate::interpreter::interpret_all(&mut env, &scope).unwrap();
    assert_eq!(
        env.traced.get(&1),
        Some(&2),
        "__trace(1) should be called twice"
    );
    assert_eq!(
        env.traced.get(&2),
        Some(&1),
        "__trace(2) should be called once"
    );
}

#[test]
fn test_builtin_assert_pass() {
    let code = r#"
func: __main() {
    __assert(1);
    __assert(42);
    return: 0;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(
        result,
        Ok(Some(0)),
        "__assert with non-zero should not panic"
    );
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_builtin_assert_fail() {
    let code = r#"
func: __main() {
    __assert(0);
    return: 0;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    crate::interpreter::interpret_all(&mut env, &scope).unwrap();
}

#[test]
fn test_builtin_puti() {
    let code = r#"
func: __main() {
    __puti(42);
    return: 0;
}
"#;
    let scope = parse_and_analyze(code);
    let (mut env, capture) = create_test_env_with_stdout_capture();
    crate::interpreter::interpret_all(&mut env, &scope).unwrap();
    env.flush();
    let output = get_stdout_from_capture(&capture);
    assert_eq!(output, "42", "__puti(42) should write '42' to stdout");
}

#[test]
fn test_builtin_putc() {
    let code = r#"
func: __main() {
    __putc(65);
    return: 0;
}
"#;
    let scope = parse_and_analyze(code);
    let (mut env, capture) = create_test_env_with_stdout_capture();
    crate::interpreter::interpret_all(&mut env, &scope).unwrap();
    env.flush();
    let output = get_stdout_from_capture(&capture);
    assert_eq!(output, "A", "__putc(65) should write 'A' to stdout");
}

#[test]
fn test_builtin_geti() {
    let code = r#"
func: __main() {
    return: __geti();
}
"#;
    let scope = parse_and_analyze(code);
    let (mut env, _) = create_test_env_with_stdin("42\n");
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(42)), "__geti() should read 42 from stdin");
}

#[test]
fn test_builtin_getc() {
    let code = r#"
func: __main() {
    return: __getc();
}
"#;
    let scope = parse_and_analyze(code);
    let (mut env, _) = create_test_env_with_stdin("A");
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(
        result,
        Ok(Some(65)),
        "__getc() should read 'A' (65) from stdin"
    );
}

// --- T2: 二項演算子テスト ---

#[test]
fn test_binary_add() {
    let code = "func: __main() { return: 1 + 2; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(3)));
}

#[test]
fn test_binary_sub() {
    let code = "func: __main() { return: 5 - 3; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(2)));
}

#[test]
fn test_binary_mul() {
    let code = "func: __main() { return: 3 * 4; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(12)));
}

#[test]
fn test_binary_div() {
    let code = "func: __main() { return: 10 / 3; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(3)));
}

#[test]
fn test_binary_mod() {
    let code = "func: __main() { return: 10 % 3; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(1)));
}

#[test]
fn test_binary_equal() {
    let code = "func: __main() { return: (3 == 3) + (3 == 4) * 10; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(1)), "3==3 is 1, 3==4 is 0");
}

#[test]
fn test_binary_not_equal() {
    let code = "func: __main() { return: (3 != 4) + (3 != 3) * 10; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(1)), "3!=4 is 1, 3!=3 is 0");
}

#[test]
fn test_binary_less() {
    let code = "func: __main() { return: (1 < 2) + (2 < 2) * 10 + (3 < 2) * 100; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(1)), "1<2 is 1, 2<2 is 0, 3<2 is 0");
}

#[test]
fn test_binary_less_equal() {
    let code = "func: __main() { return: (1 <= 2) + (2 <= 2) * 10 + (3 <= 2) * 100; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(11)), "1<=2 is 1, 2<=2 is 1, 3<=2 is 0");
}

#[test]
fn test_binary_greater() {
    let code = "func: __main() { return: (3 > 2) + (2 > 2) * 10 + (1 > 2) * 100; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(1)), "3>2 is 1, 2>2 is 0, 1>2 is 0");
}

#[test]
fn test_binary_greater_equal() {
    let code = "func: __main() { return: (3 >= 2) + (2 >= 2) * 10 + (1 >= 2) * 100; }";
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(11)), "3>=2 is 1, 2>=2 is 1, 1>=2 is 0");
}

#[test]
fn test_binary_logical_and() {
    let code = r#"
func: __main() {
    return: (1 && 1) + (1 && 0) * 10 + (0 && 1) * 100 + (0 && 0) * 1000;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(1)), "1&&1=1, 1&&0=0, 0&&1=0, 0&&0=0");
}

#[test]
fn test_binary_logical_or() {
    let code = r#"
func: __main() {
    return: (1 || 1) + (1 || 0) * 10 + (0 || 1) * 100 + (0 || 0) * 1000;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(111)), "1||1=1, 1||0=1, 0||1=1, 0||0=0");
}

// --- T3: 制御フローテスト ---

#[test]
fn test_if_else() {
    let code = r#"
func: __main() {
    let: x;
    x = if:(1) { 10; } else: { 20; };
    let: y;
    y = if:(0) { 10; } else: { 20; };
    return: x + y * 100;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(2010)), "if true -> 10, if false -> 20");
}

#[test]
fn test_while_loop() {
    let code = r#"
func: __main() {
    let: i; let: sum;
    i = 0; sum = 0;
    while:(i < 5) {
        sum = sum + i;
        i = i + 1;
    };
    return: sum;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(10)), "sum of 0..4 = 10");
}

#[test]
fn test_return_early() {
    let code = r#"
func: __main() {
    return: 42;
    return: 99;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(42)), "early return should return 42");
}

#[test]
fn test_break_in_while() {
    let code = r#"
func: __main() {
    let: i;
    i = 0;
    while:(1) {
        if:(i == 3) { break:; } else: {};
        i = i + 1;
    };
    return: i;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(3)), "break at i==3");
}

#[test]
fn test_continue_in_while() {
    let code = r#"
func: __main() {
    let: i; let: sum;
    i = 0; sum = 0;
    while:(i < 6) {
        i = i + 1;
        if:(i % 2 == 0) { continue:; } else: {};
        sum = sum + i;
    };
    return: sum;
}
"#;
    let scope = parse_and_analyze(code);
    let mut env = create_test_env();
    let result = crate::interpreter::interpret_all(&mut env, &scope);
    assert_eq!(result, Ok(Some(9)), "sum of odd numbers 1+3+5 = 9");
}
