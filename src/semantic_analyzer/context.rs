//! # AnalyzeContext
//!
//! `analyze_internal_with_parent` の引数のうち、複数の呼び出しにまたがって共有・継承される
//! コンテキスト情報を構造体にまとめたもの。
//!
//! これにより関数引数の数を削減し、コードの可読性を向上させる。

use super::function_table::FunctionTable;

/// 意味解析時に使用するコンテキスト。
///
/// `analyze_internal_with_parent` の引数のうち、スコープに依存する状態と
/// グローバルに共有される状態をまとめた構造体。
pub(super) struct AnalyzeContext<'a> {
    /// グローバル関数表（全スコープの解析で共有する）
    pub functions: &'a mut FunctionTable,
    /// 現在解析中の関数のグローバルインデックス（None = ルートスコープ・ブロックスコープ）
    pub func_global_index: Option<usize>,
}

impl<'a> AnalyzeContext<'a> {
    /// ルートスコープ用コンテキストを生成する
    pub fn new_root(functions: &'a mut FunctionTable) -> Self {
        Self {
            functions,
            func_global_index: None,
        }
    }

    /// 関数スコープ用コンテキストを生成する
    pub fn new_function(functions: &'a mut FunctionTable, func_global_index: usize) -> Self {
        Self {
            functions,
            func_global_index: Some(func_global_index),
        }
    }

    /// ブロックスコープ用コンテキストを生成する
    pub fn new_block(functions: &'a mut FunctionTable) -> Self {
        Self {
            functions,
            func_global_index: None,
        }
    }
}
