//! コード生成コンテキスト

use crate::compiler_ws::alloc_runtime::AllocRuntime;
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

/// static 変数のグローバルオフセットを計算する
///
/// 戻り値: (オフセットマップ, 合計サイズ)
fn compute_static_var_offsets(scope: &Scope) -> (HashMap<(usize, usize), i64>, i64) {
    let mut offsets = HashMap::new();
    let mut next_offset = scope.variable_count as i64; // グローバル変数の直後

    for (func_idx, func) in scope.functions.iter().enumerate() {
        for var in &func.block.scope.variables {
            if var.is_static {
                let slot_count = var.array_size.unwrap_or(1);
                offsets.insert((func_idx, var.slot_index), next_offset);
                next_offset += slot_count as i64;
            }
        }
    }

    let total_size = next_offset - scope.variable_count as i64;
    (offsets, total_size)
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

    /// ランタイムメモリアロケータ
    alloc_runtime: &'a dyn AllocRuntime,

    /// スコープオフセットスタック
    /// 各エントリは、そのスコープの変数のヒープ内ベースオフセット
    /// 末尾が現在のスコープ
    scope_offsets: Vec<i64>,

    /// 次のブロックスコープに割り当てる開始オフセット
    next_var_offset: i64,

    /// 関数内 static 変数のグローバルオフセット
    /// キー: (関数インデックス, ローカル変数スロットインデックス)
    /// 値: グローバルヒープ上のオフセット（GLOBAL_PTR からの相対）
    static_var_global_offsets: HashMap<(usize, usize), i64>,

    /// static 変数領域の合計サイズ
    static_var_total_size: i64,

    /// 現在処理中の関数インデックス (関数内でのみ Some)
    current_func_index: Option<usize>,

    /// 現在処理中の関数のスコープ (関数内でのみ Some)
    current_func_scope: Option<&'a Scope>,
}

impl<'a> CodeGenContext<'a> {
    pub fn new_with_options(
        scope: &'a Scope,
        debug_ext: bool,
        alloc_runtime: &'a dyn AllocRuntime,
    ) -> Self {
        let (static_var_global_offsets, static_var_total_size) = compute_static_var_offsets(scope);
        Self {
            scope,
            labels: LabelAllocator::new(),
            is_global: true,
            local_heap_size: 0,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext,
            alloc_runtime,
            scope_offsets: vec![0],
            next_var_offset: 0,
            static_var_global_offsets,
            static_var_total_size,
            current_func_index: None,
            current_func_scope: None,
        }
    }

    /// ローカル（関数内）コンテキストを作成
    /// total_var_count: 関数内の全ブロック（ネスト含む）の変数合計数
    /// func_scope_var_count: 関数スコープ直下の変数数
    /// func_index: 関数のインデックス
    /// func_scope: 関数のスコープ（Variable 情報参照用）
    pub fn enter_function(
        &self,
        total_var_count: usize,
        func_scope_var_count: usize,
        func_index: usize,
        func_scope: &'a Scope,
    ) -> CodeGenContext<'a> {
        CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(),
            is_global: false,
            local_heap_size: total_var_count as i64,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext: self.debug_ext,
            alloc_runtime: self.alloc_runtime,
            scope_offsets: vec![0],
            next_var_offset: func_scope_var_count as i64,
            static_var_global_offsets: self.static_var_global_offsets.clone(),
            static_var_total_size: self.static_var_total_size,
            current_func_index: Some(func_index),
            current_func_scope: Some(func_scope),
        }
    }

    /// static 変数初期化用のコンテキストを作成
    ///
    /// static 初期化では、変数への書き込みはグローバルヒープ上で行われるため、
    /// ローカルヒープの allocate/deallocate は不要。
    /// それ以外は enter_function と同じ動作でアドレス解決を行う。
    pub fn enter_function_for_static_init(
        &self,
        _total_var_count: usize,
        func_scope_var_count: usize,
        func_index: usize,
        func_scope: &'a Scope,
    ) -> CodeGenContext<'a> {
        CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(),
            is_global: false,
            local_heap_size: 0, // フレーム不要
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext: self.debug_ext,
            alloc_runtime: self.alloc_runtime,
            scope_offsets: vec![0],
            next_var_offset: func_scope_var_count as i64,
            static_var_global_offsets: self.static_var_global_offsets.clone(),
            static_var_total_size: self.static_var_total_size,
            current_func_index: Some(func_index),
            current_func_scope: Some(func_scope),
        }
    }

    /// グローバルヒープサイズを取得
    pub fn global_heap_size(&self) -> i64 {
        self.scope.variable_count as i64 + self.static_var_total_size
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
    pub fn get_function_label(&self, func_index: usize) -> Option<LabelId> {
        self.labels.get_function_label(func_index)
    }

    /// 関数ラベルを取得または作成
    pub fn get_or_create_function_label(&mut self, func_index: usize) -> LabelId {
        self.labels.get_or_create_function_label(func_index)
    }

    /// 変数参照からアドレス情報を取得
    pub fn get_var_info(&self, var_ref: &IdentifierRef) -> VarInfo {
        if var_ref.is_global {
            VarInfo {
                scope: VarScope::Global,
                offset: var_ref.local_index as i64,
            }
        } else {
            // static 変数チェック（関数スコープ直下の変数のみ）
            // owning_func_index がある場合は親関数の static 変数へのアクセス
            // （ネストされた関数から親関数の static 変数にアクセスする場合）
            // owning_func_index がない場合は current_func_index を使用
            if let (Some(func_idx), Some(_func_scope)) =
                (self.current_func_index, self.current_func_scope)
            {
                let lookup_func_idx = var_ref.owning_func_index.unwrap_or(func_idx);
                // scope_depth == 0 で関数スコープ直下の変数を参照している場合、
                // または scope_depth >= scope_offsets.len() で親関数の変数を参照している場合
                if var_ref.scope_depth == 0 || var_ref.scope_depth >= self.scope_offsets.len() {
                    // var_ref.local_index はスロットインデックスなので、
                    // static_var_global_offsets で検索
                    if let Some(global_offset) = self
                        .static_var_global_offsets
                        .get(&(lookup_func_idx, var_ref.local_index))
                    {
                        return VarInfo {
                            scope: VarScope::Global,
                            offset: *global_offset,
                        };
                    }
                }
            }

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

    /// ランタイムメモリアロケータを取得
    pub fn alloc_runtime(&self) -> &dyn AllocRuntime {
        self.alloc_runtime
    }
}

// 注: CodeGenContext のテストは、Scope のプライベートフィールドのため、
// 簡単なダミーインスタンスを作成できない。
// label.rs の LabelAllocator のテストで十分にカバーされているため、
// context.rs には統合テストのみに依存する。
