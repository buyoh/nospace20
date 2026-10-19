use std::collections::BTreeMap;
use std::io::{BufRead, Read, Write};

use super::allocator::InterpreterAllocator;
use crate::base::error::interpret_error::InterpretError;

/// NospaceVM のフレーム数上限の既定値
///
/// フレームはヒープ上に確保される（1 フレーム 88 バイト）。関数呼び出し 1 段あたり数フレームを使う。
/// 深い再帰や無限再帰でホスト（特に WASM）のメモリを使い尽くすのを防ぐ。
pub const DEFAULT_MAX_VM_FRAMES: usize = 1_000_000;

/// インタプリタの実行制限設定
pub struct EnvironmentConfig {
    /// Expression評価の最大実行回数 (Noneの場合は無制限)
    pub max_expression_count: Option<usize>,
    /// デバッグ用組み込み関数を無視する
    pub ignore_debug: bool,
    /// 未初期化変数をランダム値で埋める（初期値依存のバグ検出用）
    pub randomize_uninit: bool,
    /// NospaceVM のフレーム数上限（None の場合は無制限）。超過すると `InterpretError::StackOverflow`
    /// （ツリー走査インタプリタでは使用しない）
    pub max_vm_frames: Option<usize>,
}

impl EnvironmentConfig {
    pub fn new() -> Self {
        EnvironmentConfig {
            max_expression_count: None,
            ignore_debug: false,
            randomize_uninit: false,
            max_vm_frames: Some(DEFAULT_MAX_VM_FRAMES),
        }
    }

    pub fn with_max_expression_count(max_count: usize) -> Self {
        EnvironmentConfig {
            max_expression_count: Some(max_count),
            ..Self::new()
        }
    }
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// インタプリタの実行メトリクス
pub struct EnvironmentMetrics {
    expression_count: usize,
}

impl EnvironmentMetrics {
    pub fn new() -> Self {
        EnvironmentMetrics {
            expression_count: 0,
        }
    }

    pub fn expression_count(&self) -> usize {
        self.expression_count
    }
}

impl Default for EnvironmentMetrics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Environment {
    pub traced: BTreeMap<i64, i64>,
    pub(crate) stdin: Box<dyn BufRead>,
    pub(crate) stdout: Box<dyn Write>,
    pub config: EnvironmentConfig,
    metrics: EnvironmentMetrics,
    /// メモリアロケータ（全メモリを管理）
    pub(crate) allocator: InterpreterAllocator,
    /// グローバル変数のベースアドレス（アロケータ上）
    pub(crate) global_base_addr: i64,
    /// 関数内 static 変数のベースアドレス
    /// 関数インデックス → アロケータ上のベースアドレス
    pub(crate) function_static_addrs: BTreeMap<usize, i64>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin: Box::new(std::io::BufReader::new(std::io::stdin())),
            stdout: Box::new(std::io::stdout()),
            config: EnvironmentConfig::new(),
            metrics: EnvironmentMetrics::new(),
            allocator: InterpreterAllocator::new(),
            global_base_addr: 0,
            function_static_addrs: BTreeMap::new(),
        }
    }

    pub fn new_with_buffers(stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin,
            stdout,
            config: EnvironmentConfig::new(),
            metrics: EnvironmentMetrics::new(),
            allocator: InterpreterAllocator::new(),
            global_base_addr: 0,
            function_static_addrs: BTreeMap::new(),
        }
    }

    pub fn new_with_config(
        stdin: Box<dyn BufRead>,
        stdout: Box<dyn Write>,
        config: EnvironmentConfig,
    ) -> Self {
        Environment {
            traced: BTreeMap::new(),
            stdin,
            stdout,
            config,
            metrics: EnvironmentMetrics::new(),
            allocator: InterpreterAllocator::new(),
            global_base_addr: 0,
            function_static_addrs: BTreeMap::new(),
        }
    }

    /// 式評価回数を加算し、`max_expression_count` を超えたら `ExpressionLimitExceeded`
    pub(super) fn increment_expression_count(&mut self) -> Result<(), InterpretError> {
        self.metrics.expression_count += 1;
        match self.config.max_expression_count {
            Some(max) if self.metrics.expression_count > max => {
                Err(InterpretError::ExpressionLimitExceeded(max))
            }
            _ => Ok(()),
        }
    }

    pub fn metrics(&self) -> &EnvironmentMetrics {
        &self.metrics
    }

    // バグ修正: 従来は書き込み・flush の失敗を unwrap しており、出力先が閉じられると
    // （例: `nospace20 prog.ns | head`）BrokenPipe で panic していた。
    // 解決策: 失敗を InterpretError::Io として返す。
    pub fn write_int(&mut self, val: i64) -> Result<(), InterpretError> {
        write!(self.stdout, "{}", val).map_err(io_error)
    }

    pub fn write_char(&mut self, val: i64) -> Result<(), InterpretError> {
        // バグ修正: 以前は `(val as u8) as char` を write! しており、128..=255 が
        // Latin-1 文字として UTF-8 2 バイトで出力されていた。encode_putc で生バイト/UTF-8 を出し分ける。
        let bytes =
            crate::base::putc::encode_putc(val).ok_or(InterpretError::InvalidCharCode(val))?;
        self.stdout.write_all(&bytes).map_err(io_error)
    }

    pub fn flush(&mut self) -> Result<(), InterpretError> {
        self.stdout.flush().map_err(io_error)
    }

    pub fn read_int(&mut self) -> i64 {
        let mut buf = String::new();
        let mut chars_read = 0;
        let mut negative = false;
        let mut num_str = String::new();

        // 空白・改行をスキップして数値を読み取る
        loop {
            buf.clear();
            match self.stdin.read_line(&mut buf) {
                Ok(0) => return 0, // EOF
                Ok(_) => {
                    for ch in buf.chars() {
                        if chars_read == 0 && (ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t')
                        {
                            continue; // 先頭の空白をスキップ
                        }
                        if chars_read == 0 && ch == '-' {
                            negative = true;
                            chars_read += 1;
                            continue;
                        }
                        if ch.is_ascii_digit() {
                            num_str.push(ch);
                            chars_read += 1;
                        } else if chars_read > 0 {
                            // 数値の終わり
                            break;
                        }
                    }
                    if chars_read > 0 {
                        break;
                    }
                }
                Err(_) => return 0,
            }
        }

        let result = num_str.parse::<i64>().unwrap_or(0);
        if negative {
            -result
        } else {
            result
        }
    }

    pub fn read_char(&mut self) -> i64 {
        let mut buf = [0u8; 1];
        match self.stdin.read(&mut buf) {
            Ok(1) => buf[0] as i64,
            _ => 0, // EOF
        }
    }
}

fn io_error(e: std::io::Error) -> InterpretError {
    InterpretError::Io(e.to_string())
}
