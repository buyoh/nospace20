//! Whitespace コードの実行の組み立て
//!
//! `whitespace20`・WASM・テストが共通に使う「VM の構築」と「最後まで実行」を提供する。
//! WASM はステップ実行のため `build_vm` だけを使う。

use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, Write};

use crate::whitespace::{ParseError, RuntimeError, StepResult, WhitespaceVM};

/// VM の設定
///
/// `whitespace20` のオプション・WASM の引数・テストの条件から作る。
#[derive(Debug, Clone, Default)]
pub struct WsRunOptions {
    /// デバッグ拡張 API（`__trace` / `__assert` / `__clog` など）を有効にする
    pub debug_ext: bool,
    /// VM が使うメモリ（ヒープ・データスタック・戻り先の合計セル数）の上限。None は無制限
    pub max_memory_cells: Option<usize>,
    /// 未初期化ヒープの読み出しをエラーにする
    pub strict_heap: bool,
    /// 未初期化ヒープの読み出しで決定的な非 0 の値を返す（初期値 0 依存のバグの検出用）
    pub randomize_heap: bool,
}

/// Whitespace のソースから、`options` を適用した VM を作る
///
/// I/O（stdin / stdout / clog writer）は呼び出し側が設定する。
pub fn build_vm(ws_source: &str, options: &WsRunOptions) -> Result<WhitespaceVM, ParseError> {
    Ok(WhitespaceVM::from_source(ws_source)?
        .with_debug_ext(options.debug_ext)
        .with_max_memory_cells(options.max_memory_cells)
        .with_strict_heap(options.strict_heap)
        .with_randomize_heap(options.randomize_heap))
}

/// `run_ws_to_end` で使う I/O
pub struct RunIo {
    pub stdin: Box<dyn BufRead>,
    pub stdout: Box<dyn Write>,
    /// `__clog` の出力先。None なら捨てる
    pub clog: Option<Box<dyn Write>>,
}

/// `run_ws_to_end` が正常終了したときの VM の状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsRunOutcome {
    /// `__trace` の記録（キー → 回数）
    pub traced: BTreeMap<i64, i64>,
    /// 実行した命令数
    pub total_steps: usize,
}

/// `run_ws_to_end` の失敗
#[derive(Debug)]
pub enum WsRunError {
    /// Whitespace のソースのパースに失敗した
    Parse(ParseError),
    /// 実行時エラー
    Runtime(RuntimeError),
    /// 実行命令数の上限に達した（値は上限）
    StepLimitExceeded(usize),
    /// 入力待ちで止まった（`BufRead` の stdin では起きない）
    UnexpectedInputWait,
    /// 終了時の stdout の flush に失敗した
    Flush(std::io::Error),
}

impl fmt::Display for WsRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse error: {}", e),
            Self::Runtime(e) => write!(f, "runtime error: {}", e),
            Self::StepLimitExceeded(steps) => {
                write!(f, "execution limit exceeded ({} steps)", steps)
            }
            Self::UnexpectedInputWait => write!(f, "unexpected input wait"),
            Self::Flush(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for WsRunError {}

/// Whitespace のソースを VM で最後まで（最大 `max_steps` 命令）実行する
///
/// `build_vm` → I/O の設定 → `run_vm_to_end` をまとめたもの。
pub fn run_ws_to_end(
    ws_source: &str,
    options: &WsRunOptions,
    max_steps: usize,
    io: RunIo,
) -> Result<WsRunOutcome, WsRunError> {
    let mut vm = build_vm(ws_source, options)
        .map_err(WsRunError::Parse)?
        .with_io(io.stdin, io.stdout);
    if let Some(clog) = io.clog {
        vm = vm.with_clog_writer(clog);
    }
    run_vm_to_end(&mut vm, max_steps)?;
    Ok(WsRunOutcome {
        traced: vm.traced.clone(),
        total_steps: vm.total_steps(),
    })
}

/// 構築済みの VM を最後まで（最大 `max_steps` 命令）実行する
///
/// 結果にかかわらず、それまでの出力を失わないよう stdout を flush する。
/// 実行時エラーなどで止まった場合はそちらを返し、flush の失敗は捨てる。
/// 失敗した場合も VM の状態（`traced` など）は呼び出し側で参照できる。
pub fn run_vm_to_end(vm: &mut WhitespaceVM, max_steps: usize) -> Result<(), WsRunError> {
    let result = vm.run(max_steps);
    let flushed = vm.try_flush();
    match result {
        StepResult::Complete => flushed.map_err(WsRunError::Flush),
        StepResult::Error(e) => Err(WsRunError::Runtime(e)),
        StepResult::Suspended => Err(WsRunError::StepLimitExceeded(max_steps)),
        StepResult::WaitingForInput(_) => Err(WsRunError::UnexpectedInputWait),
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
