//! コンパイルプロパティ
//!
//! CLI 引数から構築され、各処理段階に渡される設定情報。

use std::fmt;

use crate::WsOutputFormat;

/// 言語サブセット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageStd {
    /// 全ての機能が有効（デフォルト）
    #[default]
    Standard,
    // /// 最小限の機能セット（未実装）
    // Min,
}

/// コンパイルターゲット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompileTarget {
    /// Whitespace へコンパイル
    #[default]
    Ws,
    /// ニーモニック表記へコンパイル
    Mnemonic,
}

impl CompileTarget {
    /// このターゲットに対応する Whitespace の出力形式
    pub fn output_format(self) -> WsOutputFormat {
        match self {
            CompileTarget::Ws => WsOutputFormat::Whitespace,
            CompileTarget::Mnemonic => WsOutputFormat::Mnemonic,
        }
    }
}

/// ターゲット拡張
///
/// コンパイル時に有効化する追加の拡張機能。複数同時指定可能。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetExtension {
    /// デバッグ拡張
    Debug,
    /// メモリアロケータ拡張
    Alloc,
}

/// コンパイルオプションのバリデーションエラー
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// 未対応の機能（CLI の `--target json` など）
    UnimplementedFeature(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::UnimplementedFeature(feature) => {
                write!(f, "{} is not yet implemented", feature)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// コンパイルプロパティ
///
/// CLI 引数から構築され、各処理段階に渡される設定情報。
#[derive(Debug, Clone, Default)]
pub struct CompileProperty {
    /// 言語サブセット
    pub std: LanguageStd,
    /// 指定されたコンパイルターゲット（None なら既定の Ws）
    pub target: Option<CompileTarget>,
    /// ターゲット拡張（複数指定可能）
    pub target_extensions: Vec<TargetExtension>,
    /// 出力ファイルパス（None なら stdout）
    pub output: Option<String>,
    /// デバッグモード（出力ファイル名を stderr に表示する）
    pub debug: bool,
}

impl CompileProperty {
    /// 指定されたコンパイルターゲット。未指定なら既定値（Ws）
    pub fn target_or_default(&self) -> CompileTarget {
        self.target.unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "compile_property_tests.rs"]
mod tests;
