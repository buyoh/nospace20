//! # AnalyzeContext
//!
//! `analyze_internal_with_parent` の引数のうち、複数の呼び出しにまたがって共有・継承される
//! コンテキスト情報を構造体にまとめたもの。
//!
//! これにより関数引数の数を削減し、コードの可読性を向上させる。

use super::analysis_tables::AnalysisTables;

/// 意味解析時に使用するコンテキスト。
///
/// `analyze_internal_with_parent` の引数のうち、全スコープの解析で共有する状態をまとめた構造体。
/// 解析位置に依存する状態（実行文脈）は `ScopeResolver::exec` が持つ。
pub(super) struct AnalyzeContext<'a> {
    /// 全スコープの解析で共有する表（関数表・グローバル領域の割り当て・static の初期化文）
    pub tables: &'a mut AnalysisTables,
}

impl<'a> AnalyzeContext<'a> {
    pub fn new(tables: &'a mut AnalysisTables) -> Self {
        Self { tables }
    }
}
