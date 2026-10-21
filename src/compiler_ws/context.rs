//! コード生成コンテキスト

use crate::base::SourceLocation;
use crate::compiler_ws::alloc_runtime::AllocRuntime;
use crate::compiler_ws::{global_layout::GlobalLayout, label::LabelAllocator, types::LabelId};
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

    /// アロケータ拡張が有効か (--std-ext alloc)
    alloc_ext: bool,

    /// ランタイムメモリアロケータ
    alloc_runtime: &'a dyn AllocRuntime,

    /// スコープオフセットスタック
    /// 各エントリは、そのスコープの変数のヒープ内ベースオフセット
    /// 末尾が現在のスコープ
    scope_offsets: Vec<i64>,

    /// 次のブロックスコープに割り当てる開始オフセット
    next_var_offset: i64,

    /// グローバル領域の配置（static 変数・初期化文が使う変数のオフセット）
    layout: &'a GlobalLayout,

    /// 現在処理中の関数インデックス (関数内でのみ Some)
    current_func_index: Option<usize>,

    /// 現在処理中の関数のスコープ (関数内でのみ Some)
    current_func_scope: Option<&'a Scope>,

    /// 現在処理中の文のソース位置
    /// generate_statement 内でセットされ、コンパイルエラー発生時に参照される。
    /// None の場合、位置不明エラーとなる。
    current_location: Option<SourceLocation>,
}

impl<'a> CodeGenContext<'a> {
    pub fn new_with_options(
        scope: &'a Scope,
        layout: &'a GlobalLayout,
        debug_ext: bool,
        alloc_ext: bool,
        alloc_runtime: &'a dyn AllocRuntime,
    ) -> Self {
        Self {
            scope,
            labels: LabelAllocator::new(),
            is_global: true,
            local_heap_size: 0,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext,
            alloc_ext,
            alloc_runtime,
            scope_offsets: vec![0],
            // バグ: ルートレベルのブロック変数が 0 から積まれ、グローバル変数と重なっていた。
            // 解決策: GlobalLayout が予約したルート初期化文用の領域から積む。
            next_var_offset: layout.root_init_block_range().start,
            layout,
            current_func_index: None,
            current_func_scope: None,
            current_location: None,
        }
    }
    /// local_heap_size: 関数内の全ブロック（ネスト含む）の変数合計数（i64 に収まることを検査済み）
    /// func_scope_var_count: 関数スコープ直下の変数数（local_heap_size 以下）
    /// func_index: 関数のインデックス
    /// func_scope: 関数のスコープ（Variable 情報参照用）
    pub fn enter_function(
        &self,
        local_heap_size: i64,
        func_scope_var_count: usize,
        func_index: usize,
        func_scope: &'a Scope,
    ) -> CodeGenContext<'a> {
        CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(),
            is_global: false,
            local_heap_size,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext: self.debug_ext,
            alloc_ext: self.alloc_ext,
            alloc_runtime: self.alloc_runtime,
            scope_offsets: vec![0],
            // func_scope_var_count ≤ local_heap_size（i64 に収まることを呼び出し元で検査済み）
            next_var_offset: i64::try_from(func_scope_var_count)
                .expect("function scope size is bounded by the checked frame size"),
            layout: self.layout,
            current_func_index: Some(func_index),
            current_func_scope: Some(func_scope),
            current_location: None,
        }
    }

    /// static 変数初期化用のコンテキストを作成
    ///
    /// static 初期化では、変数への書き込みはグローバルヒープ上で行われるため、
    /// ローカルヒープの allocate/deallocate は不要（`heap[LOCAL_HEAP_BEGIN]` は GLOBAL_PTR のまま）。
    /// 関数スコープ直下の非 static 変数・初期化子内のブロック変数は、
    /// GlobalLayout が予約したフレーム相当領域に配置する。
    /// それ以外は enter_function と同じ動作でアドレス解決を行う。
    pub fn enter_function_for_static_init(
        &self,
        func_index: usize,
        func_scope: &'a Scope,
    ) -> CodeGenContext<'a> {
        // バグ: scope_offsets = [0]、next_var_offset = 関数スコープ直下の変数数 から積んでいたため、
        // 非 static 変数・ブロック変数が GLOBAL_PTR + slot に置かれ、グローバル変数・static 変数・
        // アロケータの管理領域と重なっていた。
        // 解決策: GlobalLayout が関数ごとに予約した領域を、関数フレームと同じ並びで使う。
        let frame = self
            .layout
            .static_init_frame(func_index)
            .expect("static init frame must be reserved for a function with static initializers");
        CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(),
            is_global: false,
            local_heap_size: 0, // フレーム不要
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            debug_ext: self.debug_ext,
            alloc_ext: self.alloc_ext,
            alloc_runtime: self.alloc_runtime,
            scope_offsets: vec![frame.frame_base],
            next_var_offset: frame.block_base,
            layout: self.layout,
            current_func_index: Some(func_index),
            current_func_scope: Some(func_scope),
            current_location: None,
        }
    }

    /// グローバル領域の配置を取得
    pub fn layout(&self) -> &'a GlobalLayout {
        self.layout
    }

    /// 次のブロックスコープに割り当てる開始オフセットを取得（予約領域の検査用）
    pub fn next_var_offset(&self) -> i64 {
        self.next_var_offset
    }

    /// グローバルヒープサイズを取得
    pub fn global_heap_size(&self) -> i64 {
        self.layout.total_size()
    }

    /// 現在処理中の文の位置をセットする
    ///
    /// `statement.rs` で各 `LocatedExecStatement` を処理する前に呼び出す。
    /// セットした位置はコンパイルエラー発生時に使用される。
    pub fn set_location(&mut self, loc: &SourceLocation) {
        self.current_location = Some(loc.clone());
    }

    /// 現在処理中の文の位置を取得する
    ///
    /// コンパイルエラー生成時に `CompileError::with_location` の引数として使用する。
    pub fn current_location(&self) -> Option<SourceLocation> {
        self.current_location.clone()
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
    ///
    /// `local_index as i64` と `base_offset + local_index` は、変数が置かれる領域（関数フレーム・
    /// グローバル領域）の大きさが i64 に収まることを検査済みのため、溢れない。
    pub fn get_var_info(&self, var_ref: &IdentifierRef) -> VarInfo {
        if var_ref.is_global {
            VarInfo {
                scope: VarScope::Global,
                offset: var_ref.local_index as i64,
            }
        } else {
            // static 変数チェック（関数スコープ直下の変数のみ）
            //
            // バグ: `scope_depth == 0` を「関数スコープ直下」として static を検索していた。
            // scope_depth は現在のスコープからの段数なので、ブロック内から static を参照すると
            // static 領域でなくフレーム上のアドレスになり、ブロック変数のスロット番号が
            // static と一致すると static と誤認していた。
            // 解決策: 関数スコープ（scope_offsets[0]）の段 `scope_depth == len - 1` で検索する。
            if let (Some(func_idx), Some(_func_scope)) =
                (self.current_func_index, self.current_func_scope)
            {
                let lookup_func_idx = match var_ref.owning_func_index {
                    // 関数境界を越えた参照（ネストされた関数から親関数の static 変数）
                    Some(owner) => Some(owner),
                    // 自関数の関数スコープ直下の変数
                    None if var_ref.scope_depth + 1 == self.scope_offsets.len() => Some(func_idx),
                    None => None,
                };
                // var_ref.local_index はスロットインデックス
                if let Some(global_offset) = lookup_func_idx
                    .and_then(|idx| self.layout.static_var_offset(idx, var_ref.local_index))
                {
                    return VarInfo {
                        scope: VarScope::Global,
                        offset: global_offset,
                    };
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
    ///
    /// ブロックの変数は、関数内では関数フレーム（`calculate_total_variable_count`）、
    /// ルート・static 初期化子では `GlobalLayout` の予約領域に数えられており、
    /// どちらも i64 に収まることを検査済みのため、ここでは失敗しない。
    pub fn enter_block_scope(&mut self, block_var_count: usize) {
        self.scope_offsets.push(self.next_var_offset);
        self.next_var_offset = i64::try_from(block_var_count)
            .ok()
            .and_then(|count| self.next_var_offset.checked_add(count))
            .expect("block offsets are bounded by the checked frame / global layout size");
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

    /// アロケータ拡張が有効かどうかを取得
    pub fn is_alloc_ext(&self) -> bool {
        self.alloc_ext
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
