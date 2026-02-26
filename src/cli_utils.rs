//! CLI 共通ユーティリティ
//!
//! `nospace20` バイナリと `ws_profiler` サンプルで共通に使う
//! CLI 引数型を定義する。

use clap::{Args, ValueEnum};

use crate::{LanguageStd, TargetExtension};

/// 言語サブセット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum CliStd {
    #[default]
    Standard,
    Min,
    Ws,
}

impl From<CliStd> for LanguageStd {
    fn from(cli: CliStd) -> Self {
        match cli {
            CliStd::Standard => LanguageStd::Standard,
            CliStd::Min => LanguageStd::Min,
            CliStd::Ws => LanguageStd::Ws,
        }
    }
}

/// ターゲット拡張
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTargetExt {
    Debug,
    Alloc,
}

impl From<CliTargetExt> for TargetExtension {
    fn from(cli: CliTargetExt) -> Self {
        match cli {
            CliTargetExt::Debug => TargetExtension::Debug,
            CliTargetExt::Alloc => TargetExtension::Alloc,
        }
    }
}

/// コンパイル共通引数（`nospace20` と `ws_profiler` で共有）
///
/// `#[command(flatten)]` でそれぞれの `Args` 構造体に埋め込んで使う。
#[derive(Debug, Args)]
pub struct CliCompileArgs {
    /// Language subset
    #[arg(long, value_enum, default_value_t = CliStd::Standard)]
    pub std: CliStd,

    /// Standard extensions (can be specified multiple times)
    #[arg(long = "std-ext", value_enum)]
    pub std_ext: Vec<CliTargetExt>,

    /// Optimization level (0 = none, 1 = all optimizations)
    #[arg(long, default_value_t = 0)]
    pub opt: u8,
}
