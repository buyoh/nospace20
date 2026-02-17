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
    #[cfg(test)]
    pub fn has_function(&self, name: &str) -> bool {
        self.function_labels.contains_key(name)
    }

    /// 関数ラベルを取得（存在する場合）
    pub fn get_function_label(&self, name: &str) -> Option<LabelId> {
        self.function_labels.get(name).copied()
    }

    /// 子アロケータで消費されたラベル ID を同期する。
    /// 子アロケータの next_id が自身より大きい場合に更新する。
    /// また、子アロケータで作成された関数ラベルを親にマージする。
    pub fn sync_next_id(&mut self, other: &LabelAllocator) {
        if other.next_id > self.next_id {
            self.next_id = other.next_id;
        }
        // 子コンテキストで作成された関数ラベルをマージ
        for (name, label) in &other.function_labels {
            self.function_labels.entry(name.clone()).or_insert(*label);
        }
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

    #[test]
    fn test_sync_next_id() {
        let mut parent = LabelAllocator::new();
        parent.allocate(); // next_id = 17

        let mut child = parent.clone();
        child.allocate(); // child next_id = 18
        child.allocate(); // child next_id = 19

        parent.sync_next_id(&child);
        assert_eq!(parent.allocate().0, 19); // 同期後は 19 から割り当て
    }

    #[test]
    fn test_sync_function_labels() {
        let mut parent = LabelAllocator::new();
        parent.get_or_create_function_label("main"); // label_16

        let mut child = parent.clone();
        child.get_or_create_function_label("helper"); // label_18

        parent.sync_next_id(&child);

        // 子で作成された関数ラベルが親にマージされることを確認
        assert_eq!(parent.get_function_label("helper"), Some(LabelId(18)));
        // 次のラベルは 20 から
        assert_eq!(parent.allocate().0, 20);
    }

    #[test]
    fn test_sync_no_change_when_child_behind() {
        let mut parent = LabelAllocator::new();
        parent.allocate(); // next_id = 17
        parent.allocate(); // next_id = 18
        parent.allocate(); // next_id = 19

        let child = parent.clone();
        // 子では何も割り当てない (next_id = 19 のまま)

        parent.sync_next_id(&child);
        // 子の next_id が親と同じなので変化なし
        assert_eq!(parent.allocate().0, 19);
    }

    #[test]
    fn test_sync_multiple_children() {
        let mut parent = LabelAllocator::new();
        parent.allocate(); // next_id = 17

        // 第1の子コンテキスト
        let mut child1 = parent.clone();
        child1.allocate(); // child1 next_id = 18
        parent.sync_next_id(&child1);

        // 第2の子コンテキスト
        let mut child2 = parent.clone();
        child2.allocate(); // child2 next_id = 19
        child2.allocate(); // child2 next_id = 20
        parent.sync_next_id(&child2);

        // 最終的に child2 の next_id まで同期される
        assert_eq!(parent.allocate().0, 20);
    }

    #[test]
    fn test_multi_function_simulation() {
        // 実際のバグシナリオをシミュレーション:
        // 関数1で制御構造のラベルを使用し、その後関数2を定義
        let mut parent = LabelAllocator::new();

        // 関数1 (puts) を定義開始
        let func1_label = parent.get_or_create_function_label("puts"); // label_16, 17
        assert_eq!(func1_label.0, 16);

        // 関数1の本体を生成 (子コンテキスト)
        let mut child1 = parent.clone();
        let loop_start = child1.allocate(); // label_18 (while ループ開始)
        let loop_end = child1.allocate(); // label_19 (while ループ終了)
        assert_eq!(loop_start.0, 18);
        assert_eq!(loop_end.0, 19);

        // 関数1完了後、ラベルを同期 ← これがバグ修正
        parent.sync_next_id(&child1);

        // 関数2 (main) を定義
        let func2_label = parent.get_or_create_function_label("main"); // label_20, 21
        assert_eq!(func2_label.0, 20); // label_18 ではなく label_20 (重複回避成功!)

        // 関数2の本体
        let mut child2 = parent.clone();
        let if_else_label = child2.allocate(); // label_22
        assert_eq!(if_else_label.0, 22);

        parent.sync_next_id(&child2);

        // 最終的に全てのラベルが一意であることを確認
        assert_eq!(parent.allocate().0, 23);
    }
}
