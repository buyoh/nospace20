//! メモリアロケータテスト用の JSON テスト仕様の構造体定義

use serde::Deserialize;

/// テスト仕様全体
#[derive(Debug, Deserialize)]
pub struct AllocTestSpec {
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(default)]
    pub config: AllocTestConfig,
    pub vars: Vec<String>,
    pub steps: Vec<AllocStep>,
    pub check: AllocCheck,
}

/// テスト設定
#[derive(Debug, Deserialize)]
pub struct AllocTestConfig {
    #[serde(default)]
    pub global_heap_size: i64,
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    /// アロケータ種別: "bump" (デフォルト) または "fsba"
    #[serde(default = "default_allocator")]
    pub allocator: String,
}

impl Default for AllocTestConfig {
    fn default() -> Self {
        Self {
            global_heap_size: 0,
            max_steps: default_max_steps(),
            allocator: default_allocator(),
        }
    }
}

fn default_allocator() -> String {
    "bump".to_string()
}

fn default_max_steps() -> usize {
    100000
}

/// テストステップ
#[derive(Debug, Deserialize)]
#[serde(tag = "op")]
#[serde(rename_all = "snake_case")]
pub enum AllocStep {
    Alloc { var: String, size: i64 },
    Free { var: String },
    LoadPrint { var: String },
    Print { value: i64 },
    AssertVarNe { var1: String, var2: String },
    HeapPrint { address: i64 },
    Loop { count: i64, body: Vec<AllocStep> },
}

/// 検証方法
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum AllocCheck {
    AllocIo {
        stdout: String,
    },
    AllocRuntimeError {
        #[allow(dead_code)]
        error: String,
    },
}
