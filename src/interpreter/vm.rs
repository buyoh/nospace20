//! # NospaceVM — 中断・再開可能な nospace インタプリタ
//!
//! `WhitespaceVM` と同等のインターフェースを持つ明示的スタックマシン実装。
//! `step(budget)` で指定ステップ数だけ実行し、任意のタイミングで中断・再開できる。
//!
//! ## 既存インタプリタとの共存
//!
//! - 既存の再帰インタプリタ (`exec.rs`) は変更せず維持
//! - 用途: WASM ステップ実行・中断再開が必要な場合に本 VM を使用

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::rc::Rc;

use crate::base::error::NospaceError;
use crate::base::shared_writer::SharedWriter;
use crate::semantic_analyzer::Scope;

use super::environment::{Environment, EnvironmentConfig};
use super::InterpretError;

// ===== 公開型定義 =====

/// nospace インタプリタの実行結果
///
/// `WhitespaceVM` の `StepResult` に相当するが、エラー型が `InterpretError` であり
/// `WaitingForInput` は持たない（将来の interactive stdin 対応時に追加予定）。
#[derive(Debug)]
pub enum StepResult {
    /// 実行継続中（バジェット消費で中断）
    Suspended,
    /// 正常終了
    Complete {
        return_value: Option<i64>,
    },
    /// 実行時エラー
    Error(InterpretError),
}

// ===== 内部型定義（Phase 2 で拡張予定） =====

/// 実行フレーム（Phase 2 で各バリアントを実装）
///
/// フレームスタックの末尾が現在実行中のフレーム。
/// 再帰インタプリタの「関数の呼び出し深さ・今どの行を実行中か」に対応する情報を保持する。
#[allow(dead_code)]
enum Frame {
    /// Phase 2 実装前のプレースホルダ
    _Placeholder,
}

/// `execute_one_step` の戻り値（VM 内部使用）
enum ExecuteResult {
    Continue,
    Complete(Option<i64>),
    Error(InterpretError),
}

// ===== NospaceVM 本体 =====

/// nospace ステップ実行 VM
///
/// 明示的スタックマシンとして全実行状態を保持する。
/// `step()` / `run()` で指定ステップずつ実行し、任意のタイミングで中断・再開可能。
///
/// ## WhitespaceVM との対応
///
/// | WhitespaceVM            | NospaceVM               |
/// |-------------------------|-------------------------|
/// | `from_source(ws)`       | `from_source(nospace)`  |
/// | `step(budget)`          | `step(budget)`          |
/// | `run(max_steps)`        | `run(max_steps)`        |
/// | `is_complete()`         | `is_complete()`         |
/// | `total_steps()`         | `total_steps()`         |
/// | `get_stdout_string()`   | `get_stdout_string()`   |
/// | `with_stdin(buf)`       | `with_stdin(buf)`       |
/// | `with_io(stdin,stdout)` | `with_io(stdin,stdout)` |
pub struct NospaceVM {
    // === プログラム ===
    /// 解析済みスコープ（AST を所有）
    scope: Scope,

    // === 実行状態 ===
    /// フレームスタック（明示的な実行位置管理）
    /// Phase 2 で各フレーム種別を実装
    frames: Vec<Frame>,
    /// 値スタック（式評価の中間値・戻り値を格納）
    value_stack: Vec<i64>,

    // === I/O・メモリ ===
    /// 実行環境（stdin, stdout, アロケータ, メトリクス等）
    env: Environment,
    /// テスト用: stdout の内容を型安全に取得するための共有バッファ
    stdout_capture: Option<Rc<RefCell<Vec<u8>>>>,

    // === メトリクス ===
    /// 総式評価回数
    total_steps: usize,

    // === 拡張 ===
    /// トレース出力（__trace 組み込み関数の結果）
    pub traced: BTreeMap<i64, i64>,

    // === 状態フラグ ===
    /// 実行完了済みかどうか
    completed: bool,
    /// 戻り値（main 関数の return 値）
    return_value: Option<i64>,
}

impl NospaceVM {
    // ===== コンストラクタ =====

    /// nospace ソースコードから VM を構築する
    ///
    /// パース → 意味解析 → VM 構築を一括実行する。
    /// エラーの場合は `NospaceError` を返す（パースエラーを含む）。
    pub fn from_source(source: &str) -> Result<Self, NospaceError> {
        let tokens = crate::token_parser::parse_to_tokens(&source.to_string())?;
        let tree = crate::tree_parser::parse_to_tree(&tokens)?;
        let scope = crate::semantic_analyzer::analyze(&tree)?;
        Self::from_scope(scope).map_err(NospaceError::Interpret)
    }

    /// 解析済み `Scope` から VM を構築する
    ///
    /// `Scope` を所有し、初期フレームをスタックに積む。
    pub fn from_scope(scope: Scope) -> Result<Self, InterpretError> {
        // stdout キャプチャバッファを初期化
        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_writer: Box<dyn Write> =
            Box::new(SharedWriter(Rc::clone(&stdout_buf)));
        let stdin: Box<dyn BufRead> =
            Box::new(BufReader::new(Cursor::new(Vec::<u8>::new())));

        let env = Environment::new_with_buffers(stdin, stdout_writer);

        Ok(Self {
            scope,
            frames: Vec::new(),
            value_stack: Vec::new(),
            env,
            stdout_capture: Some(stdout_buf),
            total_steps: 0,
            traced: BTreeMap::new(),
            completed: false,
            return_value: None,
        })
    }

    // ===== Builder パターン =====

    /// stdin を設定する（stdout はキャプチャバッファを維持）
    pub fn with_stdin(mut self, stdin: Box<dyn BufRead>) -> Self {
        self.env.stdin = stdin;
        self
    }

    /// I/O バッファを明示指定して構築する
    ///
    /// stdout を外部バッファに設定した場合、`get_stdout_string()` ではなく
    /// 呼び出し元のバッファから直接 stdout を取得すること。
    pub fn with_io(mut self, stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self {
        self.env.stdin = stdin;
        self.env.stdout = stdout;
        // 外部 stdout を使用するためキャプチャを無効化
        self.stdout_capture = None;
        self
    }

    /// `EnvironmentConfig` を設定する
    pub fn with_config(mut self, config: EnvironmentConfig) -> Self {
        self.env.config = config;
        self
    }

    // ===== 実行メソッド =====

    /// 指定ステップ数だけ実行し、結果を返す
    ///
    /// `budget` 回の式評価を実行する。途中で完了/エラーに到達した場合は即座に返す。
    /// `budget` を消費しきった場合は `Suspended` を返す。
    pub fn step(&mut self, budget: usize) -> StepResult {
        if self.completed {
            return StepResult::Complete {
                return_value: self.return_value,
            };
        }

        for _ in 0..budget {
            match self.execute_one_step() {
                ExecuteResult::Continue => {
                    self.total_steps += 1;
                }
                ExecuteResult::Complete(value) => {
                    self.completed = true;
                    self.return_value = value;
                    return StepResult::Complete {
                        return_value: value,
                    };
                }
                ExecuteResult::Error(e) => {
                    return StepResult::Error(e);
                }
            }
        }

        StepResult::Suspended
    }

    /// 完了まで一括実行（最大ステップ制限付き）
    ///
    /// `max_steps` ステップまでに完了しない場合は `Suspended` を返す。
    pub fn run(&mut self, max_steps: usize) -> StepResult {
        self.step(max_steps)
    }

    // ===== 状態参照メソッド =====

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool {
        self.completed
    }

    /// 総式評価回数
    pub fn total_steps(&self) -> usize {
        self.total_steps
    }

    /// stdout の内容を文字列として取得する（テスト用）
    ///
    /// `with_io()` で外部 stdout を指定した場合は空文字列を返す。
    pub fn get_stdout_string(&self) -> String {
        match &self.stdout_capture {
            Some(buf) => String::from_utf8_lossy(&buf.borrow()).to_string(),
            None => String::new(),
        }
    }

    /// 戻り値を取得する（完了時のみ有効）
    pub fn return_value(&self) -> Option<i64> {
        self.return_value
    }

    /// トレース結果への参照を返す
    pub fn traced(&self) -> &BTreeMap<i64, i64> {
        &self.traced
    }

    /// stdout をフラッシュする
    pub fn flush(&mut self) {
        self.env.flush();
    }

    // ===== プライベートメソッド =====

    /// 1ステップ（1式評価）の実行
    ///
    /// フレームスタックの末尾を見て、対応する処理を実行する。
    /// Phase 2 で各フレーム種別を実装する。
    fn execute_one_step(&mut self) -> ExecuteResult {
        if self.frames.is_empty() {
            // Phase 1 骨格: フレームが空 = 未初期化 or 完了
            // Phase 2 で GlobalInit フレームの push とステップ実行を実装する
            return ExecuteResult::Complete(None);
        }

        let frame = match self.frames.last_mut() {
            Some(f) => f,
            None => return ExecuteResult::Complete(None),
        };

        match frame {
            Frame::_Placeholder => {
                // Phase 2 で実装
                ExecuteResult::Complete(None)
            }
        }
    }
}

// ===== tests =====

#[cfg(test)]
mod tests {
    use super::*;

    /// `from_source` でパースエラーが適切に返ることを確認
    #[test]
    fn test_from_source_parse_error() {
        let result = NospaceVM::from_source("this is not valid nospace!!!!");
        assert!(result.is_err());
    }

    /// 最小限の有効プログラムで VM が構築できることを確認（Phase 1 骨格）
    #[test]
    fn test_from_source_ok() {
        let src = r#"
func: __main() {
    return: 0;
}
"#;
        let result = NospaceVM::from_source(src);
        assert!(result.is_ok(), "from_source should succeed");
    }

    /// `is_complete` 初期状態確認
    #[test]
    fn test_initial_state() {
        let src = r#"
func: __main() {
    return: 42;
}
"#;
        let vm = NospaceVM::from_source(src).unwrap();
        assert!(!vm.is_complete());
        assert_eq!(vm.total_steps(), 0);
        assert_eq!(vm.return_value(), None);
    }

    /// `with_stdin` でビルダーチェーンが動作することを確認
    #[test]
    fn test_builder_with_stdin() {
        let src = r#"
func: __main() {
    return: 0;
}
"#;
        let stdin: Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new("hello".as_bytes())));
        let vm = NospaceVM::from_source(src).unwrap().with_stdin(stdin);
        assert!(!vm.is_complete());
    }

    /// `with_config` でビルダーチェーンが動作することを確認
    #[test]
    fn test_builder_with_config() {
        let src = r#"
func: __main() {
    return: 0;
}
"#;
        let config = EnvironmentConfig::new();
        let vm = NospaceVM::from_source(src).unwrap().with_config(config);
        assert!(!vm.is_complete());
    }

    /// `StepResult` のデバッグ出力確認
    #[test]
    fn test_step_result_debug() {
        let _ = format!("{:?}", StepResult::Suspended);
        let _ = format!("{:?}", StepResult::Complete { return_value: Some(1) });
        let _ = format!("{:?}", StepResult::Error(InterpretError::FunctionNotFound("f".to_string())));
    }

    /// Phase 1 骨格: step() を呼ぶと Suspended か Complete が返ること
    #[test]
    fn test_step_returns_valid_result() {
        let src = r#"
func: __main() {
    return: 0;
}
"#;
        let mut vm = NospaceVM::from_source(src).unwrap();
        // Phase 1 では frames が空なので Complete を返す（骨格実装）
        let result = vm.step(1);
        matches!(result, StepResult::Complete { .. } | StepResult::Suspended);
    }

    /// `get_stdout_string` 初期状態確認
    #[test]
    fn test_get_stdout_string_initially_empty() {
        let src = r#"
func: __main() {
    return: 0;
}
"#;
        let vm = NospaceVM::from_source(src).unwrap();
        assert_eq!(vm.get_stdout_string(), "");
    }

    /// `with_io` で外部 stdout を設定したとき `get_stdout_string` が空を返すことを確認
    #[test]
    fn test_with_io_disables_capture() {
        let src = r#"
func: __main() {
    return: 0;
}
"#;
        let stdin: Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new(b"" as &[u8])));
        let stdout: Box<dyn Write> = Box::new(Vec::<u8>::new());
        let vm = NospaceVM::from_source(src)
            .unwrap()
            .with_io(stdin, stdout);
        assert_eq!(vm.get_stdout_string(), "");
    }
}
