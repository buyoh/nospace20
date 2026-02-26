//! CLI 共通ユーティリティ
//!
//! `nospace20` バイナリと `ws_profiler` サンプルで共通に使う
//! CLI 引数型を定義する。

use clap::{Args, ValueEnum};

use crate::{LanguageStd, OptimizationOptions, TargetExtension};

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

/// 最適化パス名
///
/// `--opt` オプションで指定できる個別の最適化パス。
/// `all` を指定するとすべての実用パスを有効化する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliOptPass {
    /// すべての実用的な最適化パスを有効化
    All,
    /// 条件式最適化: If/While の条件式を Zero/Negative モードに変換
    ConditionOpt,
    /// geti/getc 最適化: 一時領域経由の入力を直接代入に変換
    GetiOpt,
    /// 定数畳み込み: コンパイル時に評価可能な定数式を Factor に置換
    ConstantFolding,
    /// 未到達関数削除: main から到達不可能な関数をダミーに置換
    DeadCode,
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

    /// Enable optimization passes (can be specified multiple times; use 'all' to enable everything)
    #[arg(long = "opt", value_enum)]
    pub opt: Vec<CliOptPass>,
}

impl CliCompileArgs {
    /// CLI 引数から `OptimizationOptions` を構築する
    pub fn build_optimization_options(&self) -> OptimizationOptions {
        if self.opt.is_empty() {
            return OptimizationOptions::none();
        }
        if self.opt.contains(&CliOptPass::All) {
            return OptimizationOptions::all();
        }
        OptimizationOptions {
            noop_test_pass: false,
            condition_opt: self.opt.contains(&CliOptPass::ConditionOpt),
            geti_opt: self.opt.contains(&CliOptPass::GetiOpt),
            constant_folding: self.opt.contains(&CliOptPass::ConstantFolding),
            dead_code: self.opt.contains(&CliOptPass::DeadCode),
        }
    }
}
