//! コンパイルプロパティ
//!
//! CLI 引数から構築され、各処理段階に渡される設定情報。

use std::fmt;

use crate::{OptimizationOptions, WsCompileOptions, WsOutputFormat};

/// 言語サブセット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageStd {
    /// 全ての機能が有効（デフォルト）
    #[default]
    Standard,
    // /// 最小限の機能セット（未実装）
    // Min,
}

/// 実行モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// Whitespace にコンパイルし、内蔵の Whitespace VM で実行する（デフォルト）
    #[default]
    Run,
    /// コンパイルモード
    Compile,
}

/// コンパイルターゲット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompileTarget {
    /// Whitespace へコンパイル
    #[default]
    Ws,
    /// ニーモニック表記へコンパイル
    Mnemonic,
    /// 中間表現 (JSON) へコンパイル（未対応）
    Json,
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

/// コンパイルプロパティのバリデーションエラー
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// 未対応の言語サブセット
    UnsupportedStd(LanguageStd),
    /// ターゲットと言語サブセットの不整合
    IncompatibleOptions {
        target: CompileTarget,
        std: LanguageStd,
    },
    /// 未対応の機能
    UnimplementedFeature(String),
    /// 拡張が現在のモード/設定では使用不可
    InvalidExtension(String),
    /// コンパイルモード専用のオプションが他のモードで指定された（値はオプション名）
    CompileOnlyOption(&'static str),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::UnsupportedStd(std) => {
                write!(f, "--std={:?} is not yet implemented", std)
            }
            ValidationError::IncompatibleOptions { target, std: _ } => {
                write!(
                    f,
                    "--target={:?} requires --std=ws\n  tip: use `--std=ws --mode=compile --target={:?}`",
                    target, target
                )
            }
            ValidationError::UnimplementedFeature(feature) => {
                write!(f, "{} is not yet implemented", feature)
            }
            ValidationError::InvalidExtension(msg) => {
                write!(f, "{}", msg)
            }
            ValidationError::CompileOnlyOption(name) => {
                write!(f, "{} is only valid with --mode=compile", name)
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
    /// 実行モード
    pub mode: ExecutionMode,
    /// 指定されたコンパイルターゲット（mode=Compile 時のみ有効。None なら既定の Ws）
    pub target: Option<CompileTarget>,
    /// ターゲット拡張（複数指定可能）
    ///
    /// mode=Run では alloc だけをこの値に従わせ、debug は `ignore_debug` で決める（`run_compile_options`）
    pub target_extensions: Vec<TargetExtension>,
    /// 出力ファイルパス（mode=Compile 時、None なら stdout）
    pub output: Option<String>,
    /// デバッグモード
    pub debug: bool,
    /// デバッグ用組み込み関数を無視する
    pub ignore_debug: bool,
}

impl CompileProperty {
    /// 指定されたコンパイルターゲット。未指定なら既定値（Ws）
    pub fn target_or_default(&self) -> CompileTarget {
        self.target.unwrap_or_default()
    }

    /// `--mode run` で使うコンパイルオプション
    ///
    /// debug 拡張は `ignore_debug` がなければ常に有効にする（`target_extensions` に Debug が
    /// あっても `ignore_debug` を優先する）。alloc 拡張は `target_extensions` に従う。
    /// 実行する VM の debug 拡張の有無には、返した値の `debug_ext` を使う
    pub fn run_compile_options(&self, optimization: OptimizationOptions) -> WsCompileOptions {
        WsCompileOptions {
            debug_ext: !self.ignore_debug,
            alloc_ext: self.target_extensions.contains(&TargetExtension::Alloc),
            output_format: WsOutputFormat::Whitespace,
            optimization,
        }
    }

    /// バリデーション
    pub fn validate(&self) -> Result<(), ValidationError> {
        // BUG FIX: run モードで指定された --target / --output が黙って無視されていた。
        // 明示指定かどうかを区別できるよう target を Option にし、ここでエラーにする。
        if self.mode == ExecutionMode::Run {
            if self.target.is_some() {
                return Err(ValidationError::CompileOnlyOption("--target"));
            }
            if self.output.is_some() {
                return Err(ValidationError::CompileOnlyOption("--output"));
            }
        }

        // コンパイルモードの場合
        if self.mode == ExecutionMode::Compile {
            // 未対応のターゲット
            match self.target_or_default() {
                CompileTarget::Json => {
                    return Err(ValidationError::UnimplementedFeature(
                        "--target=json".to_string(),
                    ));
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "compile_property_tests.rs"]
mod tests;
