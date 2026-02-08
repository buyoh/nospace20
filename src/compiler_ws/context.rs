//! コード生成コンテキスト

use crate::compiler_ws::{label::LabelAllocator, types::LabelId};
use crate::semantic_analyzer::{IdentifierRef, Scope};
use std::collections::HashMap;

/// 変数のスコープ種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarScope {
    Global,
    Local,
}

/// 変数情報
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub scope: VarScope,
    pub offset: i64,
}

/// コード生成コンテキスト
#[derive(Clone)]
pub struct CodeGenContext<'a> {
    /// 元の Scope 構造
    scope: &'a Scope,

    /// ラベル管理
    labels: LabelAllocator,

    /// 現在のスコープがグローバルか
    is_global: bool,

    /// 現在の関数のローカル変数サイズ
    local_heap_size: i64,

    /// 変数マッピング (変数名 → VarInfo)
    /// ※ 現在は使用していないが、将来的な拡張用に保持
    #[allow(dead_code)]
    variables: HashMap<String, VarInfo>,
}

impl<'a> CodeGenContext<'a> {
    pub fn new(scope: &'a Scope) -> Self {
        Self {
            scope,
            labels: LabelAllocator::new(),
            is_global: true,
            local_heap_size: 0,
            variables: HashMap::new(),
        }
    }

    /// ローカル（関数内）コンテキストを作成
    pub fn enter_function(&self, local_var_count: usize) -> CodeGenContext<'a> {
        CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(),
            is_global: false,
            local_heap_size: local_var_count as i64,
            variables: HashMap::new(),
        }
    }

    /// グローバルヒープサイズを取得
    pub fn global_heap_size(&self) -> i64 {
        self.scope.variable_count as i64
    }

    /// 新しいラベルを確保
    pub fn new_label(&mut self) -> LabelId {
        self.labels.allocate()
    }

    /// ラベル範囲を確保
    pub fn new_label_range(&mut self, count: u32) -> LabelId {
        self.labels.allocate_range(count)
    }

    /// 関数ラベルを取得
    pub fn get_function_label(&self, name: &str) -> Option<LabelId> {
        self.labels.get_function_label(name)
    }

    /// 関数ラベルを取得または作成
    pub fn get_or_create_function_label(&mut self, name: &str) -> LabelId {
        self.labels.get_or_create_function_label(name)
    }

    /// 変数参照からアドレス情報を取得
    pub fn get_var_info(&self, var_ref: &IdentifierRef) -> VarInfo {
        if var_ref.is_global {
            VarInfo {
                scope: VarScope::Global,
                offset: var_ref.local_index as i64,
            }
        } else {
            VarInfo {
                scope: VarScope::Local,
                offset: var_ref.local_index as i64,
            }
        }
    }

    /// 現在のスコープを取得
    pub fn scope(&self) -> &'a Scope {
        self.scope
    }

    /// ローカルヒープサイズを取得
    pub fn local_heap_size(&self) -> i64 {
        self.local_heap_size
    }
}
