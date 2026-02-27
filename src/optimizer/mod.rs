//! # Optimizer
//!
//! 意味解析後の中間表現 (`Scope`) に対して最適化パスを適用するモジュール。
//!
//! ## 設計
//!
//! - 各最適化パスはプラグイン形式で、`OptimizationOptions` により個別に有効化・無効化できる
//! - パスは `&mut Scope` を受け取り、中間表現を直接変換する
//! - パス適用後は通常通り `&Scope` として Interpreter / Compiler WS に渡される
//!
//! ## パスの実行順序
//!
//! 1. noop_test_pass (テスト用: マジックナンバー変数を追加)
//! 2. constant_folding
//! 3. condition_opt
//! 4. geti_opt
//! 5. dead_code

mod condition_opt;
mod constant_folding;
mod dead_code;
mod geti_opt;
mod noop_test_pass;

#[cfg(test)]
mod tests;

use crate::semantic_analyzer::Scope;

/// 最適化オプション
///
/// 各最適化パスの有効化・無効化を制御する。
/// CLI の `--opt` オプションから構築される。
#[derive(Debug, Clone)]
pub struct OptimizationOptions {
    /// テスト用パス: マジックナンバー変数を追加する
    /// 実用的な最適化ではなく、フレームワークの動作検証に使用
    pub noop_test_pass: bool,
    /// 条件式最適化パス: If/While の条件式を ConditionMode(Zero/Negative) に変換する
    pub condition_opt: bool,
    /// geti/getc 最適化パス: `p = __geti()` / `p = __getc()` を InternalBuiltinFunction に変換する
    pub geti_opt: bool,
    /// 定数畳み込みパス: コンパイル時に評価可能な定数式を Factor に置換する
    pub constant_folding: bool,
    /// 未到達関数削除パス: main から到達不可能な関数をダミーに置換する
    pub dead_code: bool,
    /// ピープホール最適化パス: 生成済み Whitespace 命令列に対して冗長パターンを除去する
    /// 中間表現レベルではなく WsProgram レベルで適用される（compiler_ws 内で処理）
    pub peephole: bool,
}

impl OptimizationOptions {
    /// 最適化なし
    pub fn none() -> Self {
        Self {
            noop_test_pass: false,
            constant_folding: false,
            condition_opt: false,
            geti_opt: false,
            dead_code: false,
            peephole: false,
        }
    }

    /// 全最適化（テスト用パスを除く）
    pub fn all() -> Self {
        Self {
            noop_test_pass: false,
            constant_folding: true,
            condition_opt: true,
            geti_opt: true,
            dead_code: true,
            peephole: true,
        }
    }

    /// 意味解析レベルのいずれかの最適化が有効かどうか
    /// ※ peephole は WsProgram レベルのため含まない
    pub fn any_enabled(&self) -> bool {
        self.noop_test_pass || self.constant_folding || self.condition_opt || self.geti_opt || self.dead_code
    }
}

impl Default for OptimizationOptions {
    fn default() -> Self {
        Self::none()
    }
}

/// Scope に対して最適化パスを適用する
///
/// `options` で有効化されたパスのみ実行される。
/// パスの実行順序は固定されており、依存関係を考慮している。
pub fn optimize(scope: &mut Scope, options: &OptimizationOptions) {
    if !options.any_enabled() {
        return;
    }

    // テスト用パス
    if options.noop_test_pass {
        noop_test_pass::apply(scope);
    }

    // 定数畳み込みパス（条件式最適化の前に実行）
    if options.constant_folding {
        constant_folding::apply(scope);
    }

    // 条件式最適化パス
    if options.condition_opt {
        condition_opt::apply(scope);
    }

    // geti/getc 最適化パス
    if options.geti_opt {
        geti_opt::apply(scope);
    }

    // 未到達関数削除パスは最後に実行（他パス完了後の状態を基に判定）
    if options.dead_code {
        dead_code::apply(scope);
    }
}
