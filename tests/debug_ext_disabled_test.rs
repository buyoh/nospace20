//! debug 拡張なし（`--std-ext debug` を付けない）でのコンパイル・実行の結合テスト
//!
//! debug 組み込み関数（`__assert` / `__assert_not` / `__trace` / `__clog`）は、副作用だけを残して無視される。
//! 実行は `whitespace20` と同じ `runner::run_ws_to_end` を通す（`tests/common/ns_run.rs`）。

mod common;

use common::ns_run::compile_and_run;
use nospace20::runner::WsRunError;
use nospace20::whitespace::RuntimeError;

#[test]
fn test_debug_ext_disabled_assert_does_not_fail() {
    // __assert(0) は通常エラーになるが、debug 拡張なしでは無視される
    let source = r#"
        func: __main() {
            __assert(0);
            __puti(42);
        }
    "#;

    let run = compile_and_run(source, false);
    assert!(run.result.is_ok(), "{:?}", run.result.err());
    assert_eq!(run.stdout, "42");
}

#[test]
fn test_debug_ext_disabled_assert_not_does_not_fail() {
    // __assert_not(1) は通常エラーになるが、debug 拡張なしでは無視される
    let source = r#"
        func: __main() {
            __assert_not(1);
            __puti(99);
        }
    "#;

    let run = compile_and_run(source, false);
    assert!(run.result.is_ok(), "{:?}", run.result.err());
    assert_eq!(run.stdout, "99");
}

#[test]
fn test_debug_ext_disabled_preserves_side_effects() {
    // 副作用（代入）は debug 拡張なしでも保持される
    let source = r#"
        let: a;
        func: __main() {
            a = 0;
            __assert(a = a + 2);
            __puti(a);
        }
    "#;

    let run = compile_and_run(source, false);
    assert!(run.result.is_ok(), "{:?}", run.result.err());
    // a = 0 + 2 = 2 になっているはず
    assert_eq!(run.stdout, "2");
}

#[test]
fn test_debug_ext_disabled_trace_does_not_record() {
    // __trace は debug 拡張なしでは記録されない
    let source = r#"
        func: __main() {
            __trace(1);
            __trace(2);
            __puti(0);
        }
    "#;

    let run = compile_and_run(source, false);
    assert!(run.result.is_ok(), "{:?}", run.result.err());
    assert!(run.result.unwrap().is_empty());
    assert_eq!(run.stdout, "0");
}

#[test]
fn test_debug_ext_disabled_clog_does_not_print() {
    // __clog は debug 拡張なしでは clog writer にも stdout にも出力されない
    let source = r#"
        func: __main() {
            let: x;
            x = 42;
            __clog(x);
            __puti(x);
        }
    "#;

    let run = compile_and_run(source, false);
    assert!(run.result.is_ok(), "{:?}", run.result.err());
    assert_eq!(run.clog, "");
    assert_eq!(run.stdout, "42");
}

#[test]
fn test_normal_clog_prints_to_clog_writer() {
    // debug 拡張ありでは __clog は clog writer に出力され、stdout には混ざらない
    let source = r#"
        func: __main() {
            __clog(42);
            __puti(1);
        }
    "#;

    let run = compile_and_run(source, true);
    assert!(run.result.is_ok(), "{:?}", run.result.err());
    assert_eq!(run.clog, "__clog: 42\n");
    assert_eq!(run.stdout, "1");
}

#[test]
fn test_normal_assert_fails() {
    // debug 拡張ありでは __assert(0) は AssertionFailed で止まる
    let source = r#"
        func: __main() {
            __assert(0);
            __puti(1);
        }
    "#;

    let run = compile_and_run(source, true);
    assert_matches_assertion_failed(&run.result);
    assert_eq!(run.stdout, "");
}

#[test]
fn test_normal_assert_not_fails() {
    // debug 拡張ありでは __assert_not(1) は AssertionFailed で止まる
    let source = r#"
        func: __main() {
            __assert_not(1);
            __puti(1);
        }
    "#;

    let run = compile_and_run(source, true);
    assert_matches_assertion_failed(&run.result);
    assert_eq!(run.stdout, "");
}

#[test]
fn test_normal_trace_records() {
    // debug 拡張ありでは __trace は記録される
    let source = r#"
        func: __main() {
            __trace(0);
            __trace(1);
            __trace(0);
        }
    "#;

    let run = compile_and_run(source, true);
    let traced = run.result.unwrap();
    assert_eq!(traced.get(&0), Some(&2)); // trace(0) が 2 回
    assert_eq!(traced.get(&1), Some(&1)); // trace(1) が 1 回
}

fn assert_matches_assertion_failed<T: std::fmt::Debug>(result: &Result<T, WsRunError>) {
    assert!(
        matches!(
            result,
            Err(WsRunError::Runtime(RuntimeError::AssertionFailed(_)))
        ),
        "expected AssertionFailed, got {:?}",
        result
    );
}
