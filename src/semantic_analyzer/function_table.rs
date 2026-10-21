//! # FunctionTable
//!
//! 意味解析中のグローバル関数表。ルート・関数・ブロックのすべてのスコープの解析で共有する。

use super::scope::Function;
use super::types::{Block, ValueType};

/// 意味解析中のグローバル関数表
///
/// ネスト関数・ブロック内の関数を含む全関数を、宣言を見つけた順に登録する。
/// 登録順のインデックス（グローバル関数インデックス）が `IdentifierRef::local_index` などから参照される。
pub(super) struct FunctionTable {
    functions: Vec<Function>,
    /// マングル済みの関数名（`functions` と同じ順）。同名の関数が複数あってもよい
    names: Vec<String>,
    /// `functions[i].return_type` と同じ値。`infer_type` に `&[ValueType]` で渡すため別に持つ
    return_types: Vec<ValueType>,
}

impl FunctionTable {
    pub(super) fn new() -> Self {
        Self {
            functions: Vec::new(),
            names: Vec::new(),
            return_types: Vec::new(),
        }
    }

    /// 関数宣言のスキャン（`scan_function_declarations`）: 関数を登録し、グローバル関数インデックスを返す。
    /// 本体は空のブロック
    pub(super) fn declare(&mut self, mangled_name: String, return_type: ValueType) -> usize {
        let index = self.functions.len();
        self.functions.push(Function {
            return_type: return_type.clone(),
            is_unused: false,
            ..Function::dummy()
        });
        self.names.push(mangled_name);
        self.return_types.push(return_type);
        index
    }

    /// パス 2: 本体の解析結果を設定する。戻り値型は declare 時の値のまま（変えない）
    pub(super) fn define(&mut self, index: usize, arg_indices: Vec<usize>, block: Block) {
        let function = &mut self.functions[index];
        function.arg_indices = arg_indices;
        function.block = block;
        debug_assert_eq!(function.return_type, self.return_types[index]);
    }

    pub(super) fn return_type(&self, index: usize) -> &ValueType {
        &self.return_types[index]
    }

    pub(super) fn return_types(&self) -> &[ValueType] {
        &self.return_types
    }

    /// `ScopeBuilder::build` に渡すため、関数と関数名に分解する
    pub(super) fn into_parts(self) -> (Vec<Function>, Vec<String>) {
        (self.functions, self.names)
    }
}
