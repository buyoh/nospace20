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

/// ループのジャンプ先と、ループに入った時点のスタックの状態（break / continue 用）
#[derive(Debug, Clone, Copy)]
struct LoopFrame {
    /// continue のジャンプ先
    start: LabelId,
    /// break のジャンプ先
    end: LabelId,
    /// ループに入った時点の `pending_stack_values`
    pending_stack_values: usize,
}

/// コード生成コンテキスト
#[derive(Clone)]
pub struct CodeGenContext<'a> {
    /// 元の Scope 構造
    scope: &'a Scope,

    /// ラベル管理
    labels: LabelAllocator,

    /// 関数本体のコードを生成しているか（ルートの文・static 変数の初期化の文脈では false）
    in_function_body: bool,

    /// 現在の関数のローカル変数サイズ
    local_heap_size: i64,

    /// 変数マッピング (変数名 → VarInfo)
    /// ※ 現在は使用していないが、将来的な拡張用に保持
    #[allow(dead_code)]
    variables: HashMap<String, VarInfo>,

    /// ループラベルスタック (break/continue のため)
    loop_labels: Vec<LoopFrame>,

    /// 式の評価途中でデータスタックに積まれている中間値の個数。
    /// 関数本体の開始時は 0（プロローグが積む `old_LHB` は数えない）。
    /// return / break / continue がジャンプ前の後始末に使う。
    pending_stack_values: usize,

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
            in_function_body: false,
            local_heap_size: 0,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            pending_stack_values: 0,
            debug_ext,
            alloc_ext,
            alloc_runtime,
            scope_offsets: vec![0],
            // バグ: ルートレベルのブロック変数が 0 から積まれ、グローバル変数と重なっていた。
            // 解決策: GlobalLayout が予約したルート初期化文用の領域から積む。
            next_var_offset: layout.root_init_block_range().start,
            layout,
            current_location: None,
        }
    }
    /// local_heap_size: 関数内の全ブロック（ネスト含む）の変数合計数（i64 に収まることを検査済み）
    /// func_scope_var_count: 関数スコープ直下の変数数（local_heap_size 以下）
    pub fn enter_function(
        &self,
        local_heap_size: i64,
        func_scope_var_count: usize,
    ) -> CodeGenContext<'a> {
        CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(),
            in_function_body: true,
            local_heap_size,
            variables: HashMap::new(),
            loop_labels: Vec::new(),
            pending_stack_values: 0,
            debug_ext: self.debug_ext,
            alloc_ext: self.alloc_ext,
            alloc_runtime: self.alloc_runtime,
            scope_offsets: vec![0],
            // func_scope_var_count ≤ local_heap_size（i64 に収まることを呼び出し元で検査済み）
            next_var_offset: i64::try_from(func_scope_var_count)
                .expect("function scope size is bounded by the checked frame size"),
            layout: self.layout,
            current_location: None,
        }
    }

    /// 関数本体のコードを生成しているか
    pub fn in_function_body(&self) -> bool {
        self.in_function_body
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
        // ルートの変数・すべての static 変数はグローバル領域にある（意味解析が is_global で示す）
        //
        // バグ: 以前は関数の static を「関数スコープの段か」（scope_depth + 1 == len）と owning_func_index から
        // 推測して引いており、段数の取り違えで誤ったアドレスになることがあった。ブロックの static は
        // 引く手段が無く、フレーム上に置かれていた。
        // 解決策: static 変数を意味解析でグローバル領域に割り当て、is_global の参照として受け取る。
        if var_ref.is_global {
            return VarInfo {
                scope: VarScope::Global,
                offset: var_ref.local_index as i64,
            };
        }
        // それ以外は、現在の関数（またはルートの初期化文）のフレーム上のスコープの変数。
        // 関数境界・static の初期化子の外の非 static 変数は意味解析が拒否するので、段数は scope_offsets に収まる
        let scope_offsets_len = self.scope_offsets.len();
        let scope_idx = scope_offsets_len
            .checked_sub(1 + var_ref.scope_depth)
            .expect("non-global variable must be in a scope of the current frame");
        let base_offset = self.scope_offsets[scope_idx];
        VarInfo {
            scope: VarScope::Local,
            offset: base_offset + var_ref.local_index as i64,
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
    ///
    /// その時点の中間値の個数も記録し、break / continue が捨てる個数の基準にする。
    pub fn push_loop_labels(&mut self, loop_start: LabelId, loop_end: LabelId) {
        self.loop_labels.push(LoopFrame {
            start: loop_start,
            end: loop_end,
            pending_stack_values: self.pending_stack_values,
        });
    }

    /// ループラベルをポップ (while 式生成完了時)
    pub fn pop_loop_labels(&mut self) {
        self.loop_labels.pop();
    }

    /// 現在のループの開始ラベルを取得 (continue 用)
    pub fn current_loop_start(&self) -> Option<LabelId> {
        self.loop_labels.last().map(|frame| frame.start)
    }

    /// 現在のループの終了ラベルを取得 (break 用)
    pub fn current_loop_end(&self) -> Option<LabelId> {
        self.loop_labels.last().map(|frame| frame.end)
    }

    /// 式の評価途中でデータスタックに積まれている中間値の個数を取得
    pub fn pending_stack_values(&self) -> usize {
        self.pending_stack_values
    }

    /// 現在のループに入ってから積まれた中間値の個数を取得 (break / continue の後始末用)
    ///
    /// ループの外では None。
    pub fn pending_stack_values_in_loop(&self) -> Option<usize> {
        self.loop_labels.last().map(|frame| {
            self.pending_stack_values
                .checked_sub(frame.pending_stack_values)
                .expect("pending stack values must not drop below the loop entry")
        })
    }

    /// `n` 個の値をスタックに残したまま `f` でコードを生成する
    ///
    /// `f` の中で生成される return / break / continue は、この `n` 個も後始末の対象に数える。
    /// `f` が `Err` を返した（`?` で抜けた）場合も個数を元に戻す。
    pub fn with_pending_values<T>(&mut self, n: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        self.pending_stack_values += n;
        let result = f(self);
        self.pending_stack_values -= n;
        result
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

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
