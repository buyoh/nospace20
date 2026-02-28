#[path = "ignore_debug_test/helpers.rs"]
mod helpers;

use helpers::{interpret_func_with_config, parse_and_analyze};
use nospace20::{Environment, EnvironmentConfig};
use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

#[test]
fn test_ignore_debug_assert_does_not_panic() {
    // __assert(0) は通常パニックするが、ignore_debug=true では無視される
    let source = r#"
        func: __main() {
            __assert(0);
            __puti(42);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig {
        ignore_debug: true,
        ..Default::default()
    };

    let (_, output) = interpret_func_with_config(&scope, "__main", config);
    assert_eq!(output, "42");
}

#[test]
fn test_ignore_debug_assert_not_does_not_panic() {
    // __assert_not(1) は通常パニックするが、ignore_debug=true では無視される
    let source = r#"
        func: __main() {
            __assert_not(1);
            __puti(99);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig {
        ignore_debug: true,
        ..Default::default()
    };

    let (_, output) = interpret_func_with_config(&scope, "__main", config);
    assert_eq!(output, "99");
}

#[test]
fn test_ignore_debug_preserves_side_effects() {
    // 副作用（代入）は ignore_debug=true でも保持される
    let source = r#"
        let: a;
        func: __main() {
            a = 0;
            __assert(a = a + 2);
            __puti(a);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig {
        ignore_debug: true,
        ..Default::default()
    };

    // グローバル変数を使うので interpret_with_env を使う
    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(Vec::<u8>::new())));
    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let stdout_clone = Rc::clone(&stdout_buf);

    struct SharedWriter(Rc<RefCell<Vec<u8>>>);
    impl std::io::Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.borrow_mut().flush()
        }
    }

    let stdout_writer: Box<dyn std::io::Write> = Box::new(SharedWriter(stdout_clone));
    let mut env = Environment::new_with_config(stdin_cursor, stdout_writer, config);

    nospace20::interpret_with_env(&mut env, &scope).unwrap();

    let stdout_vec = stdout_buf.borrow().clone();
    let output = String::from_utf8(stdout_vec).unwrap();

    // a = 0 + 2 = 2 になっているはず
    assert_eq!(output, "2");
}

#[test]
fn test_ignore_debug_trace_does_not_record() {
    // __trace は ignore_debug=true で記録されない
    let source = r#"
        func: __main() {
            __trace(1);
            __trace(2);
            __puti(0);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig {
        ignore_debug: true,
        ..Default::default()
    };

    let (traced, output) = interpret_func_with_config(&scope, "__main", config);
    assert_eq!(traced.len(), 0); // traced が記録されない
    assert_eq!(output, "0");
}

#[test]
fn test_ignore_debug_clog_does_not_print() {
    // __clog は ignore_debug=true で出力されない（ただし、stdout キャプチャしていないので間接的にテスト）
    let source = r#"
        func: __main() {
            let: x;
            x = 42;
            __clog(x);
            __puti(x);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig {
        ignore_debug: true,
        ..Default::default()
    };

    let (_, output) = interpret_func_with_config(&scope, "__main", config);
    // __clog は無視されるので、stdout には 42 だけが出力される
    assert_eq!(output, "42");
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_normal_assert_panics() {
    // ignore_debug=false（デフォルト）では __assert(0) でパニックする
    let source = r#"
        func: __main() {
            __assert(0);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig::new(); // ignore_debug=false

    let _ = interpret_func_with_config(&scope, "__main", config);
    // ここでパニックするはず
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_normal_assert_not_panics() {
    // ignore_debug=false（デフォルト）では __assert_not(1) でパニックする
    let source = r#"
        func: __main() {
            __assert_not(1);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig::new(); // ignore_debug=false

    let _ = interpret_func_with_config(&scope, "__main", config);
    // ここでパニックするはず
}

#[test]
fn test_normal_trace_records() {
    // ignore_debug=false（デフォルト）では __trace は記録される
    let source = r#"
        func: __main() {
            __trace(0);
            __trace(1);
            __trace(0);
        }
    "#;

    let scope = parse_and_analyze(source);
    let config = EnvironmentConfig::new(); // ignore_debug=false

    let (traced, _) = interpret_func_with_config(&scope, "__main", config);
    assert_eq!(traced.get(&0), Some(&2)); // trace(0) が 2 回
    assert_eq!(traced.get(&1), Some(&1)); // trace(1) が 1 回
}
