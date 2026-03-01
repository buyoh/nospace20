//! コンパイルプロパティ
//!
//! CLI 引数から構築され、各処理段階に渡される設定情報。

use std::fmt;

/// 言語サブセット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageStd {
    /// 全ての機能が有効（デフォルト）
    #[default]
    Standard,
    /// 最小限の機能セット（未対応）
    Min,
    /// Whitespace コンパイル互換
    Ws,
}

/// 実行モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// インタプリタモード（デフォルト）
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
    /// コンパイルターゲット（mode=Compile 時のみ使用）
    pub target: CompileTarget,
    /// ターゲット拡張（mode=Compile 時のみ使用、複数指定可能）
    pub target_extensions: Vec<TargetExtension>,
    /// 出力ファイルパス（mode=Compile 時、None なら stdout）
    pub output: Option<String>,
    /// デバッグモード
    pub debug: bool,
    /// デバッグ用組み込み関数を無視する
    pub ignore_debug: bool,
}

impl CompileProperty {
    /// バリデーション
    pub fn validate(&self) -> Result<(), ValidationError> {
        // std=min は未対応
        if self.std == LanguageStd::Min {
            return Err(ValidationError::UnsupportedStd(self.std));
        }

        // コンパイルモードの場合
        if self.mode == ExecutionMode::Compile {
            // target=ws/mnemonic の場合、std=ws が必須
            if matches!(self.target, CompileTarget::Ws | CompileTarget::Mnemonic) {
                if self.std != LanguageStd::Ws {
                    return Err(ValidationError::IncompatibleOptions {
                        target: self.target,
                        std: self.std,
                    });
                }
            }

            // 未対応のターゲット
            match self.target {
                CompileTarget::Json => {
                    return Err(ValidationError::UnimplementedFeature(
                        "--target=json".to_string(),
                    ));
                }
                _ => {}
            }
        }

        // --std-ext alloc は --mode=compile --std=ws またはインタープリタ実行時に有効
        if self.target_extensions.contains(&TargetExtension::Alloc) {
            if self.mode == ExecutionMode::Compile && self.std != LanguageStd::Ws {
                return Err(ValidationError::InvalidExtension(
                    "--std-ext alloc requires --mode=compile --std=ws".to_string(),
                ));
            }
        }

        Ok(())
    }
}
