//! nospace → Whitespace コンパイル → WhitespaceVM 実行の結合テスト
//!
//! WhitespaceVM のメモリ上限と、__clog の debug 拡張 API (-13) を確認する。

use nospace20::whitespace::{RuntimeError, StepResult, WhitespaceVM};
use nospace20::{
    compile_to_ws, parse_to_tokens, parse_to_tree, semantic_analyze, WsCompileOptions,
    WsOutputFormat,
};
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

/// 共有バッファに書き込む writer（clog writer の出力の取得用）
///
/// `nospace20::base::shared_writer::SharedWriter` は crate 外に公開されていないため、ここで定義する
struct SharedWriter(Rc<RefCell<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn compile(source: &str, debug_ext: bool, output_format: WsOutputFormat) -> String {
    let tokens = parse_to_tokens(&source.to_string()).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = semantic_analyze(&ast).unwrap();
    compile_to_ws(
        &scope,
        &WsCompileOptions {
            debug_ext,
            output_format,
            ..Default::default()
        },
    )
    .unwrap()
}

/// 末尾でない無限再帰は、メモリ上限付きの VM で MemoryLimitExceeded になる
///
/// 自己末尾再帰（`return: f();`）は末尾呼び出し最適化でループになりうるため使わない
#[test]
fn test_non_tail_infinite_recursion_hits_memory_limit() {
    let ws = compile(
        "func: f() { return: f() + 1; } func: __main() { __puti(f()); }",
        false,
        WsOutputFormat::Whitespace,
    );
    let limit_cells = 1 << 12;
    let mut vm = WhitespaceVM::from_source(&ws)
        .unwrap()
        .with_max_memory_cells(Some(limit_cells));
    assert_eq!(
        vm.run(10_000_000),
        StepResult::Error(RuntimeError::MemoryLimitExceeded(limit_cells))
    );
    assert_eq!(vm.get_stdout_string(), "");
}

/// debug 拡張ありの __clog は clog writer に出力され、プログラムの stdout には混ざらない。
/// 戻り値は引数の値
#[test]
fn test_clog_with_debug_ext_writes_to_clog_writer() {
    let ws = compile(
        "func: __main() { let: x; x = __clog(7); __puti(x); }",
        true,
        WsOutputFormat::Whitespace,
    );
    let clog_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let mut vm = WhitespaceVM::from_source(&ws)
        .unwrap()
        .with_debug_ext(true)
        .with_clog_writer(Box::new(SharedWriter(Rc::clone(&clog_buf))));
    assert_eq!(vm.run(100_000), StepResult::Complete);
    assert_eq!(vm.get_stdout_string(), "7");
    assert_eq!(String::from_utf8_lossy(&clog_buf.borrow()), "__clog: 7\n");
}

/// debug 拡張ありの __clog は -13 への store を生成し、なしでは生成しない（noop）
#[test]
fn test_clog_compiles_to_ext_store_only_with_debug_ext() {
    let source = "func: __main() { __clog(7); }";
    let with_debug = compile(source, true, WsOutputFormat::Mnemonic);
    let without_debug = compile(source, false, WsOutputFormat::Mnemonic);
    assert!(with_debug.contains("-13"), "{}", with_debug);
    assert!(!without_debug.contains("-13"), "{}", without_debug);
}
