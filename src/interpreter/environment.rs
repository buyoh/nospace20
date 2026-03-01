use std::collections::BTreeMap;
use std::io::{BufRead, Read, Write};

use super::allocator::InterpreterAllocator;

/// インタプリタの実行制限設定
pub struct EnvironmentConfig {
    /// Expression評価の最大実行回数 (Noneの場合は無制限)
    pub max_expression_count: Option<usize>,
    /// デバッグ用組み込み関数を無視する
    pub ignore_debug: bool,
    /// 未初期化変数をランダム値で埋める（初期値依存のバグ検出用）
    pub randomize_uninit: bool,
}

impl EnvironmentConfig {
    pub fn new() -> Self {
        EnvironmentConfig {
            max_expression_count: None,
            ignore_debug: false,
            randomize_uninit: false,
        }
    }

    pub fn with_max_expression_count(max_count: usize) -> Self {
        EnvironmentConfig {
            max_expression_count: Some(max_count),
            ignore_debug: false,
            randomize_uninit: false,
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

    pub(super) fn increment_expression_count(&mut self) {
        self.metrics.expression_count += 1;
        if let Some(max) = self.config.max_expression_count {
            if self.metrics.expression_count > max {
                panic!(
                    "Expression evaluation limit exceeded: {} > {}",
                    self.metrics.expression_count, max
                );
            }
        }
    }

    pub fn metrics(&self) -> &EnvironmentMetrics {
        &self.metrics
    }

    pub fn write_int(&mut self, val: i64) {
        write!(self.stdout, "{}", val).unwrap();
    }

    pub fn write_char(&mut self, val: i64) {
        let byte = (val as u8) as char;
        write!(self.stdout, "{}", byte).unwrap();
    }

    pub fn flush(&mut self) {
        self.stdout.flush().unwrap();
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
