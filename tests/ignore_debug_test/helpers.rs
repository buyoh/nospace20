//! ignore_debug テスト用のヘルパー関数

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::rc::Rc;

use nospace20::whitespace::{StepResult, WhitespaceVM};
use nospace20::{
    compile_to_ws, parse_to_tokens, parse_to_tree, semantic_analyze, WsCompileOptions,
};

/// テストでの実行ステップの上限
const MAX_STEPS: usize = 1_000_000;

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

/// compile → WhitespaceVM で実行した結果
pub struct WsRunResult {
    pub result: StepResult,
    pub stdout: String,
    /// __clog の出力先（clog writer）に書かれた内容
    pub clog: String,
    pub traced: BTreeMap<i64, i64>,
}

/// `debug_ext` を指定してコンパイルし、同じ `debug_ext` の WhitespaceVM で実行する
///
/// `--mode run` の `--ignore-debug` は `debug_ext: false` でのコンパイル・実行に相当する
pub fn compile_and_run(source: &str, debug_ext: bool) -> WsRunResult {
    let tokens = parse_to_tokens(&source.to_string()).unwrap();
    let tree = parse_to_tree(&tokens).unwrap();
    let scope = semantic_analyze(&tree).unwrap();
    let ws = compile_to_ws(
        &scope,
        &WsCompileOptions {
            debug_ext,
            ..Default::default()
        },
    )
    .unwrap();

    let clog_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let mut vm = WhitespaceVM::from_source(&ws)
        .unwrap()
        .with_debug_ext(debug_ext)
        .with_clog_writer(Box::new(SharedWriter(Rc::clone(&clog_buf))));
    let result = vm.run(MAX_STEPS);
    vm.flush();
    let clog = String::from_utf8_lossy(&clog_buf.borrow()).to_string();
    WsRunResult {
        result,
        stdout: vm.get_stdout_string(),
        clog,
        traced: vm.traced.clone(),
    }
}
