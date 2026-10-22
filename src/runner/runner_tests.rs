use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::rc::Rc;

use super::*;
use crate::base::shared_writer::SharedWriter;
use crate::{analyze_source, compile_to_ws, OptimizationOptions, WsCompileOptions};

/// 書き込みは受け付けるが flush が常に失敗する writer（`> /dev/full` 相当）
struct FailingFlushWriter;

impl Write for FailingFlushWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("flush failed"))
    }
}

/// メモリ上のバッファに出力を集める I/O
struct BufferedIo {
    stdout: Rc<RefCell<Vec<u8>>>,
    clog: Rc<RefCell<Vec<u8>>>,
}

impl BufferedIo {
    fn new() -> Self {
        Self {
            stdout: Rc::new(RefCell::new(Vec::new())),
            clog: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn run_io(&self, stdin: &str) -> RunIo {
        RunIo {
            stdin: Box::new(Cursor::new(stdin.as_bytes().to_vec())),
            stdout: Box::new(SharedWriter(Rc::clone(&self.stdout))),
            clog: Some(Box::new(SharedWriter(Rc::clone(&self.clog)))),
        }
    }

    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.stdout.borrow()).to_string()
    }

    fn clog(&self) -> String {
        String::from_utf8_lossy(&self.clog.borrow()).to_string()
    }
}

fn compile(source: &str, debug_ext: bool) -> String {
    let scope = analyze_source(source, &OptimizationOptions::none()).unwrap();
    compile_to_ws(
        &scope,
        &WsCompileOptions {
            debug_ext,
            ..Default::default()
        },
    )
    .unwrap()
}

fn debug_options() -> WsRunOptions {
    WsRunOptions {
        debug_ext: true,
        ..Default::default()
    }
}

#[test]
fn complete_writes_stdout_and_returns_traced() {
    let ws = compile(
        "func: __main() { __trace(3); __trace(3); __puti(__geti() * 2); }",
        true,
    );
    let io = BufferedIo::new();
    let outcome = run_ws_to_end(&ws, &debug_options(), usize::MAX, io.run_io("21\n")).unwrap();
    assert_eq!(io.stdout(), "42");
    assert_eq!(outcome.traced.get(&3), Some(&2));
    assert!(outcome.total_steps > 0);
}

#[test]
fn clog_goes_to_clog_writer_not_stdout() {
    let ws = compile("func: __main() { __puti(__clog(7)); }", true);
    let io = BufferedIo::new();
    run_ws_to_end(&ws, &debug_options(), usize::MAX, io.run_io("")).unwrap();
    assert_eq!(io.stdout(), "7");
    assert_eq!(io.clog(), "__clog: 7\n");
}

/// 実行時エラーで止まっても、それまでの出力は flush される
#[test]
fn runtime_error_keeps_previous_output() {
    let ws = compile("func: __main() { __puti(1); __assert(0); }", true);
    let io = BufferedIo::new();
    let err = run_ws_to_end(&ws, &debug_options(), usize::MAX, io.run_io("")).unwrap_err();
    assert!(
        matches!(err, WsRunError::Runtime(RuntimeError::AssertionFailed(_))),
        "{:?}",
        err
    );
    assert_eq!(io.stdout(), "1");
}

#[test]
fn step_limit_is_reported() {
    let ws = compile(
        "func: __main() { let: i(0); while: 1 { i = i + 1; }; }",
        false,
    );
    let io = BufferedIo::new();
    let err = run_ws_to_end(&ws, &WsRunOptions::default(), 1000, io.run_io("")).unwrap_err();
    assert!(
        matches!(err, WsRunError::StepLimitExceeded(1000)),
        "{:?}",
        err
    );
    assert_eq!(err.to_string(), "execution limit exceeded (1000 steps)");
}

#[test]
fn invalid_whitespace_source_is_parse_error() {
    let io = BufferedIo::new();
    // Push に数値の終端（LF）がない
    let err = run_ws_to_end("   ", &WsRunOptions::default(), 100, io.run_io("")).unwrap_err();
    assert!(matches!(err, WsRunError::Parse(_)), "{:?}", err);
}

/// 正常終了時の flush の失敗はエラーとして返す（`> /dev/full` で exit 0 にしない）
#[test]
fn flush_failure_on_complete_is_error() {
    let ws = compile("func: __main() { __puti(1); }", false);
    let io = RunIo {
        stdin: Box::new(Cursor::new(Vec::new())),
        stdout: Box::new(FailingFlushWriter),
        clog: None,
    };
    let err = run_ws_to_end(&ws, &WsRunOptions::default(), usize::MAX, io).unwrap_err();
    assert!(matches!(err, WsRunError::Flush(_)), "{:?}", err);
    assert_eq!(err.to_string(), "I/O error: flush failed");
}

/// 実行時エラーと flush の失敗が重なった場合は、実行時エラーを返す
#[test]
fn runtime_error_takes_precedence_over_flush_failure() {
    let ws = compile("func: __main() { __assert(0); }", true);
    let io = RunIo {
        stdin: Box::new(Cursor::new(Vec::new())),
        stdout: Box::new(FailingFlushWriter),
        clog: None,
    };
    let err = run_ws_to_end(&ws, &debug_options(), usize::MAX, io).unwrap_err();
    assert!(matches!(err, WsRunError::Runtime(_)), "{:?}", err);
}

/// debug 拡張なしの VM では __assert は効かない（拡張 API の書き込みは通常のヒープ書き込み）
#[test]
fn debug_ext_disabled_ignores_assert_in_debug_compiled_code() {
    let ws = compile("func: __main() { __assert(0); __puti(5); }", true);
    let io = BufferedIo::new();
    run_ws_to_end(&ws, &WsRunOptions::default(), usize::MAX, io.run_io("")).unwrap();
    assert_eq!(io.stdout(), "5");
}

#[test]
fn build_vm_applies_memory_limit() {
    let ws = compile(
        "func: f() { return: f() + 1; } func: __main() { __puti(f()); }",
        false,
    );
    let options = WsRunOptions {
        max_memory_cells: Some(1 << 10),
        ..Default::default()
    };
    let mut vm = build_vm(&ws, &options).unwrap();
    assert_eq!(
        vm.run(10_000_000),
        StepResult::Error(RuntimeError::MemoryLimitExceeded(1 << 10))
    );
}
