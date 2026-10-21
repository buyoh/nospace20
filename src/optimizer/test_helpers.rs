//! optimizer のユニットテスト用ヘルパ
//!
//! 最適化パスの前後で意味が変わらないことを、Scope を Whitespace にコンパイルして
//! WhitespaceVM で実行した結果（`__trace` のヒット回数と stdout）の比較で確かめる。
//! コンパイラは前後で共通なので、差が出れば多くの場合は最適化パスが原因である。
//! ただし最適化後にだけ現れる IR はコンパイラの別の経路を通るので、各テストでは
//! 前後比較と一緒に絶対的な期待値も確かめる。

use std::collections::BTreeMap;

use crate::semantic_analyzer::Scope;
use crate::whitespace::{StepResult, WhitespaceVM};
use crate::{compile_to_ws, WsCompileOptions};

/// 実行ステップの上限（large テストと同じ）
const MAX_STEPS: usize = 1_000_000;

/// Scope を WS にコンパイルし、WhitespaceVM で実行した結果
#[derive(Debug, PartialEq, Eq)]
pub(super) struct WsRunOutcome {
    /// __trace のキーごとのヒット回数
    pub traced: BTreeMap<i64, i64>,
    /// 標準出力の内容
    pub stdout: String,
}

/// debug 拡張ありでコンパイルし、stdin を与えて実行する。
/// コンパイルエラー・実行時エラー・ステップ上限超過は panic（テストの失敗）にする
pub(super) fn run_ws(scope: &Scope, stdin: &str) -> WsRunOutcome {
    let ws = compile_to_ws(
        scope,
        &WsCompileOptions {
            debug_ext: true,
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("WS compile failed: {}", e));
    let mut vm = WhitespaceVM::from_source(&ws)
        .unwrap_or_else(|e| panic!("failed to parse compiled Whitespace: {}", e))
        .with_debug_ext(true)
        .with_stdin(Box::new(std::io::Cursor::new(stdin.as_bytes().to_vec())));
    match vm.run(MAX_STEPS) {
        StepResult::Complete => {}
        other => panic!("WS execution did not complete: {:?}", other),
    }
    WsRunOutcome {
        traced: vm.traced.clone(),
        stdout: vm.get_stdout_string(),
    }
}
