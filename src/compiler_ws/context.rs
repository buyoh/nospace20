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
    #[allow(dead_code)]
    is_global: bool,

    /// 現在の関数のローカル変数サイズ
    local_heap_size: i64,

    /// 変数マッピング (変数名 → VarInfo)
    /// ※ 現在は使用していないが、将来的な拡張用に保持
    #[allow(dead_code)]
    variables: HashMap<String, VarInfo>,

    /// ループラベルスタック (break/continue のため)
    /// (loop_start, loop_end) のペア
    loop_labels: Vec<(LabelId, LabelId)>,

    /// デバッグ拡張 API が有効か (--std-ext debug)
    debug_ext: bool,

    /// スコープオフセットスタック
    /// 各エントリは、そのスコープの変数のヒープ内ベースオフセット
    /// 末尾が現在のスコープ
    scope_offsets: Vec<i64>,

    /// 次のブロックスコープに割り当てる開始オフセット
    next_var_offset: i64,
}

impl<'a> CodeGenContext<'a> {
    pub fn new(scope: &'a Scope) -> Self {
        Self::new_with_options(scope, false)
    }

    pub fn new_with_options(scope: &'a Scope, debug_ext: bool) -> Self {
        Self {
            scope,
            labels: LabelAllocator::new(),
            is_global: true,
            local_heap_size: 0,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext,
            scope_offsets: vec![0],
            next_var_offset: 0,
        }
    }

    /// ローカル（関数内）コンテキストを作成
    /// total_var_count: 関数内の全ブロック（ネスト含む）の変数合計数
    /// func_scope_var_count: 関数スコープ直下の変数数
    pub fn enter_function(
        &self,
        total_var_count: usize,
        func_scope_var_count: usize,
    ) -> CodeGenContext<'a> {
        CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(),
            is_global: false,
            local_heap_size: total_var_count as i64,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext: self.debug_ext,
            scope_offsets: vec![0],
            next_var_offset: func_scope_var_count as i64,
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
    #[allow(dead_code)]
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
            // scope_depth が scope_offsets の範囲を超える場合はグローバル変数として扱う
            // （static 変数などで発生する可能性がある）
            let scope_offsets_len = self.scope_offsets.len();
            if var_ref.scope_depth >= scope_offsets_len {
                VarInfo {
                    scope: VarScope::Global,
                    offset: var_ref.local_index as i64,
                }
            } else {
                let scope_idx = scope_offsets_len - 1 - var_ref.scope_depth;
                let base_offset = self.scope_offsets[scope_idx];
                VarInfo {
                    scope: VarScope::Local,
                    offset: base_offset + var_ref.local_index as i64,
                }
            }
        }
    }

    /// 現在のスコープを取得
    #[allow(dead_code)]
    pub fn scope(&self) -> &'a Scope {
        self.scope
    }

    /// ローカルヒープサイズを取得
    pub fn local_heap_size(&self) -> i64 {
        self.local_heap_size
    }

    /// ループラベルをプッシュ (while 式生成時)
    pub fn push_loop_labels(&mut self, loop_start: LabelId, loop_end: LabelId) {
        self.loop_labels.push((loop_start, loop_end));
    }

    /// ループラベルをポップ (while 式生成完了時)
    pub fn pop_loop_labels(&mut self) {
        self.loop_labels.pop();
    }

    /// 現在のループの開始ラベルを取得 (continue 用)
    pub fn current_loop_start(&self) -> Option<LabelId> {
        self.loop_labels.last().map(|(start, _)| *start)
    }

    /// 現在のループの終了ラベルを取得 (break 用)
    pub fn current_loop_end(&self) -> Option<LabelId> {
        self.loop_labels.last().map(|(_, end)| *end)
    }

    /// ブロックスコープに入る
    /// block_var_count: このブロックの variable_count
    pub fn enter_block_scope(&mut self, block_var_count: usize) {
        self.scope_offsets.push(self.next_var_offset);
        self.next_var_offset += block_var_count as i64;
    }

    /// ブロックスコープから出る
    pub fn leave_block_scope(&mut self) {
        self.scope_offsets.pop();
        // next_var_offset は戻さない（各スコープに一意のオフセットを保証）
    }

    /// 子コンテキストで消費されたラベルカウンタを親に同期する。
    pub fn sync_labels_from(&mut self, child: &CodeGenContext) {
        self.labels.sync_next_id(&child.labels);
    }

    /// デバッグ拡張が有効かどうかを取得
    pub fn is_debug_ext(&self) -> bool {
        self.debug_ext
    }
}

// 注: CodeGenContext のテストは、Scope のプライベートフィールドのため、
// 簡単なダミーインスタンスを作成できない。
// label.rs の LabelAllocator のテストで十分にカバーされているため、
// context.rs には統合テストのみに依存する。
