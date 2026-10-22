//! nospace のソースを compile → WhitespaceVM で実行するテスト用ヘルパー
//!
//! ライブラリの `compile_source` / `runner::run_ws_to_end` を使う（`whitespace20` と同じ `build_vm` / `run_vm_to_end` を通る）。
//! 出力はメモリ上のバッファに集め、ファイルは作らない。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::rc::Rc;

use nospace20::runner::{run_ws_to_end, RunIo, WsRunError, WsRunOptions};
use nospace20::{compile_source, SharedWriter, WsCompileOptions};

/// テストでの実行ステップの上限
pub const MAX_STEPS: usize = 1_000_000;

/// ソースを Whitespace にコンパイルする（失敗したら panic）
pub fn compile_ns(source: &str, options: &WsCompileOptions) -> String {
    compile_source(source, options).unwrap_or_else(|e| panic!("compile failed: {}", e))
}

/// compile → WhitespaceVM で実行した結果
pub struct WsRunResult {
    /// 実行の結果（正常終了なら `__trace` の記録）
    pub result: Result<BTreeMap<i64, i64>, WsRunError>,
    pub stdout: String,
    /// __clog の出力先（clog writer）に書かれた内容
    pub clog: String,
}

/// `debug_ext` を指定してコンパイルし、同じ `debug_ext` の VM で、空の stdin で実行する
pub fn compile_and_run(source: &str, debug_ext: bool) -> WsRunResult {
    let ws = compile_ns(
        source,
        &WsCompileOptions {
            debug_ext,
            ..Default::default()
        },
    );
    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let clog_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let io = RunIo {
        stdin: Box::new(Cursor::new(Vec::new())),
        stdout: Box::new(SharedWriter(Rc::clone(&stdout_buf))),
        clog: Some(Box::new(SharedWriter(Rc::clone(&clog_buf)))),
    };
    let options = WsRunOptions {
        debug_ext,
        ..Default::default()
    };
    let result = run_ws_to_end(&ws, &options, MAX_STEPS, io).map(|outcome| outcome.traced);
    let to_string = |buf: &Rc<RefCell<Vec<u8>>>| String::from_utf8_lossy(&buf.borrow()).to_string();
    WsRunResult {
        result,
        stdout: to_string(&stdout_buf),
        clog: to_string(&clog_buf),
    }
}
