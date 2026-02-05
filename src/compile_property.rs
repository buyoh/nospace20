//! コンパイルプロパティ
//!
//! CLI 引数から構築され、各処理段階に渡される設定情報。

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
    /// 拡張 Whitespace へコンパイル（未対応）
    ExWs,
    /// 中間表現 (JSON) へコンパイル（未対応）
    Json,
}

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
    /// 出力ファイルパス（mode=Compile 時、None なら stdout）
    pub output: Option<String>,
    /// デバッグモード
    pub debug: bool,
}

impl CompileProperty {
    /// バリデーション
    pub fn validate(&self) -> Result<(), String> {
        // std=min は未対応
        if self.std == LanguageStd::Min {
            return Err("--std=min is not yet implemented".to_string());
        }
        
        // コンパイルモードの場合
        if self.mode == ExecutionMode::Compile {
            // target=ws/mnemonic の場合、std=ws が必須
            if matches!(self.target, CompileTarget::Ws | CompileTarget::Mnemonic) {
                if self.std != LanguageStd::Ws {
                    return Err(format!(
                        "--target={:?} requires --std=ws\n  tip: use `--std=ws --mode=compile --target={:?}`",
                        self.target, self.target
                    ));
                }
            }
            
            // 未対応のターゲット
            match self.target {
                CompileTarget::ExWs => {
                    return Err("--target=ex-ws is not yet implemented".to_string());
                }
                CompileTarget::Json => {
                    return Err("--target=json is not yet implemented".to_string());
                }
                _ => {}
            }
        }
        
        Ok(())
    }
}
