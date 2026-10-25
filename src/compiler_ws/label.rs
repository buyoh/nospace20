//! ラベル管理

use crate::compiler_ws::types::LabelId;
use std::collections::HashMap;

/// 予約ラベル定義
pub mod reserved_labels {
    use super::LabelId;

    // 0-11: 予備（旧: ユーザーコード開始点・組み込み比較ルーチン。番号を詰めるとユーザーラベルの採番が変わるため空けておく）

    /// ランタイムアロケータ: __rt_alloc サブルーチン
    pub const RT_ALLOC: LabelId = LabelId(12);
    /// ランタイムアロケータ: __rt_free サブルーチン
    pub const RT_FREE: LabelId = LabelId(13);

    // 14-15: テスト等で使用可能な予備
    // 16-47: FSBA アロケータ内部ラベル（alloc_runtime/fsba.rs で定義）

    /// ユーザーラベルのオフセット
    /// 注: 0-47 はシステム予約（予約ラベル + FSBA 内部ラベル）
    pub const LABEL_OFFSET: u32 = 48;
}

/// ラベル管理器
#[derive(Debug, Clone)]
pub struct LabelAllocator {
    /// 次に割り当てるラベルID
    next_id: u32,
    /// 関数インデックス → ラベルID のマッピング
    /// 関数名ではなくインデックスをキーにすることで、
    /// 同名関数のシャドーイング時にもラベルの一意性を保証する。
    function_labels: HashMap<usize, LabelId>,
}

impl LabelAllocator {
    pub fn new() -> Self {
        Self {
            next_id: reserved_labels::LABEL_OFFSET,
            function_labels: HashMap::new(),
        }
    }

    /// 新しいラベルを確保 (制御構造用)
    pub fn allocate(&mut self) -> LabelId {
        let id = LabelId(self.next_id);
        self.next_id += 1;
        id
    }

    /// 連続したラベルを確保
    /// 返り値は範囲の先頭ラベル
    pub fn allocate_range(&mut self, count: u32) -> LabelId {
        let base = LabelId(self.next_id);
        self.next_id += count;
        base
    }

    /// 関数用ラベルを取得または作成
    /// 関数は2つのラベルを使用 (エントリ点 + スキップ先)
    /// キーは関数のグローバルインデックスで、同名関数のシャドーイング時にも一意性を保証する。
    pub fn get_or_create_function_label(&mut self, func_index: usize) -> LabelId {
        if let Some(&label) = self.function_labels.get(&func_index) {
            label
        } else {
            let label = self.allocate_range(2);
            self.function_labels.insert(func_index, label);
            label
        }
    }

    /// 関数ラベルが存在するか確認
    #[cfg(test)]
    pub fn has_function(&self, func_index: usize) -> bool {
        self.function_labels.contains_key(&func_index)
    }

    /// 関数ラベルを取得（存在する場合）
    pub fn get_function_label(&self, func_index: usize) -> Option<LabelId> {
        self.function_labels.get(&func_index).copied()
    }

    /// 子アロケータで消費されたラベル ID を同期する。
    /// 子アロケータの next_id が自身より大きい場合に更新する。
    /// また、子アロケータで作成された関数ラベルを親にマージする。
    pub fn sync_next_id(&mut self, other: &LabelAllocator) {
        if other.next_id > self.next_id {
            self.next_id = other.next_id;
        }
        // 子コンテキストで作成された関数ラベルをマージ
        for (func_index, label) in &other.function_labels {
            self.function_labels.entry(*func_index).or_insert(*label);
        }
    }
}

impl Default for LabelAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "label_tests.rs"]
mod tests;
