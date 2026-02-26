//! CLI 共通ユーティリティ
//!
//! `nospace20` バイナリと `ws_profiler` サンプルで共通に使う
//! CLI 引数型を定義する。

use clap::ValueEnum;

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
