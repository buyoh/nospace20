//! # AnalyzeContext
//!
//! `analyze_internal_with_parent` の引数のうち、複数の呼び出しにまたがって共有・継承される
//! コンテキスト情報を構造体にまとめたもの。
//!
//! これにより関数引数の数を削減し、コードの可読性を向上させる。

use super::scope::Function;
use super::types::ValueType;

/// 意味解析時に使用するコンテキスト。
///
/// `analyze_internal_with_parent` の引数のうち、スコープに依存する状態と
/// グローバルに共有される状態をまとめた構造体。
pub(super) struct AnalyzeContext<'a> {
    /// グローバル関数リスト（再帰呼び出し間で共有される）
    pub global_functions: &'a mut Vec<Function>,
    /// グローバル関数名リスト（登録順保持・重複チェック用）
    pub global_function_names: &'a mut Vec<String>,
    /// 現在解析中の関数のグローバルインデックス（None = ルートスコープ）
    pub func_global_index: Option<usize>,
    /// 外側のスコープから継承した関数戻り値型。
    /// 空の場合は `global_functions` から動的に収集する。
    pub inherited_func_return_types: Vec<ValueType>,
}

impl<'a> AnalyzeContext<'a> {
    /// ルートスコープ用コンテキストを生成する
    pub fn new_root(
        global_functions: &'a mut Vec<Function>,
        global_function_names: &'a mut Vec<String>,
    ) -> Self {
        Self {
            global_functions,
            global_function_names,
            func_global_index: None,
            inherited_func_return_types: Vec::new(),
        }
    }

    /// 関数スコープ用コンテキストを生成する
    pub fn new_function(
        global_functions: &'a mut Vec<Function>,
        global_function_names: &'a mut Vec<String>,
        func_global_index: usize,
    ) -> Self {
        Self {
            global_functions,
            global_function_names,
            func_global_index: Some(func_global_index),
            inherited_func_return_types: Vec::new(),
        }
    }
}
