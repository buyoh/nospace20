//! ラベル管理

use crate::compiler_ws::types::LabelId;
use std::collections::HashMap;

/// 予約ラベル定義
pub mod reserved_labels {
    use super::LabelId;

    /// ユーザーコード開始点
    pub const USER_CODE_BEGIN: LabelId = LabelId(0);

    /// ゼロ判定ルーチン
    pub const COMPARATOR_ZERO: LabelId = LabelId(2);
    pub const COMPARATOR_ZERO_2: LabelId = LabelId(3);

    /// 負数判定ルーチン
    pub const COMPARATOR_NEGATIVE: LabelId = LabelId(4);
    pub const COMPARATOR_NEGATIVE_2: LabelId = LabelId(5);

    /// AND ルーチン
    pub const COMPARATOR_AND: LabelId = LabelId(6);
    pub const COMPARATOR_AND_2: LabelId = LabelId(7);

    /// OR ルーチン
    pub const COMPARATOR_OR: LabelId = LabelId(8);
    pub const COMPARATOR_OR_2: LabelId = LabelId(9);
    pub const COMPARATOR_OR_3: LabelId = LabelId(10);

    /// ユーザーラベルのオフセット
    pub const LABEL_OFFSET: u32 = 16;
}

/// ラベル管理器
#[derive(Debug, Clone)]
pub struct LabelAllocator {
    /// 次に割り当てるラベルID
    next_id: u32,
    /// 関数名 → ラベルID のマッピング
    function_labels: HashMap<String, LabelId>,
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
    pub fn get_or_create_function_label(&mut self, name: &str) -> LabelId {
        if let Some(&label) = self.function_labels.get(name) {
            label
        } else {
            let label = self.allocate_range(2);
            self.function_labels.insert(name.to_string(), label);
            label
        }
    }

    /// 関数ラベルが存在するか確認
    pub fn has_function(&self, name: &str) -> bool {
        self.function_labels.contains_key(name)
    }

    /// 関数ラベルを取得（存在する場合）
    pub fn get_function_label(&self, name: &str) -> Option<LabelId> {
        self.function_labels.get(name).copied()
    }
}

impl Default for LabelAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_allocator() {
        let mut alloc = LabelAllocator::new();
        let l1 = alloc.allocate();
        let l2 = alloc.allocate();
        assert_eq!(l1.0, 16);
        assert_eq!(l2.0, 17);
    }

    #[test]
    fn test_label_range() {
        let mut alloc = LabelAllocator::new();
        let base = alloc.allocate_range(5);
        assert_eq!(base.0, 16);

        let next = alloc.allocate();
        assert_eq!(next.0, 21);
    }

    #[test]
    fn test_function_label() {
        let mut alloc = LabelAllocator::new();

        let f1 = alloc.get_or_create_function_label("foo");
        assert_eq!(f1.0, 16);

        let f1_again = alloc.get_or_create_function_label("foo");
        assert_eq!(f1_again.0, 16); // 同じラベル

        let f2 = alloc.get_or_create_function_label("bar");
        assert_eq!(f2.0, 18); // foo が 16-17 を使用、bar は 18-19
    }

    #[test]
    fn test_has_function() {
        let mut alloc = LabelAllocator::new();

        assert!(!alloc.has_function("test"));
        alloc.get_or_create_function_label("test");
        assert!(alloc.has_function("test"));
    }
}
