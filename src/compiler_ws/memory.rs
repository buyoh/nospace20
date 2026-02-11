//! メモリレイアウト管理

use crate::compiler_ws::types::HeapAddress;

/// メモリレイアウト管理
///
/// Whitespace ヒープの予約領域と変数配置を管理する。
pub struct MemoryLayout {
    /// グローバル変数の数
    #[allow(dead_code)]
    global_var_count: i64,
}

impl MemoryLayout {
    /// 新しいメモリレイアウトを作成
    pub fn new() -> Self {
        Self {
            global_var_count: 0,
        }
    }

    // === 予約アドレス（定数） ===

    /// ローカルヒープの開始位置を格納するアドレス
    pub const LOCAL_HEAP_BEGIN: HeapAddress = HeapAddress(2);

    /// ローカルヒープの終了位置を格納するアドレス
    pub const LOCAL_HEAP_END: HeapAddress = HeapAddress(3);

    /// 一時ポインタ（内部使用）
    pub const TEMP_PTR: HeapAddress = HeapAddress(4);

    /// グローバル変数領域の開始アドレス
    pub const GLOBAL_PTR: HeapAddress = HeapAddress(8);

    // === 動的アドレス計算 ===

    /// グローバル変数を登録し、そのアドレスを返す
    #[allow(dead_code)]
    pub fn allocate_global(&mut self) -> HeapAddress {
        let addr = Self::GLOBAL_PTR.offset(self.global_var_count);
        self.global_var_count += 1;
        addr
    }

    /// グローバル変数領域のサイズを取得
    #[allow(dead_code)]
    pub fn global_size(&self) -> i64 {
        self.global_var_count
    }

    /// ローカルヒープ初期値（global領域の直後）
    #[allow(dead_code)]
    pub fn initial_local_heap(&self) -> HeapAddress {
        Self::GLOBAL_PTR.offset(self.global_var_count)
    }
}

impl Default for MemoryLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// 後方互換性のための定数エイリアス
pub mod heap_layout {
    use super::MemoryLayout;
    pub const LOCAL_HEAP_BEGIN: i64 = MemoryLayout::LOCAL_HEAP_BEGIN.0;
    pub const LOCAL_HEAP_END: i64 = MemoryLayout::LOCAL_HEAP_END.0;
    pub const TEMP_PTR: i64 = MemoryLayout::TEMP_PTR.0;
    pub const GLOBAL_PTR: i64 = MemoryLayout::GLOBAL_PTR.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_layout_constants() {
        assert_eq!(MemoryLayout::LOCAL_HEAP_BEGIN.value(), 2);
        assert_eq!(MemoryLayout::LOCAL_HEAP_END.value(), 3);
        assert_eq!(MemoryLayout::TEMP_PTR.value(), 4);
        assert_eq!(MemoryLayout::GLOBAL_PTR.value(), 8);
    }

    #[test]
    fn test_allocate_global() {
        let mut layout = MemoryLayout::new();

        let addr1 = layout.allocate_global();
        assert_eq!(addr1.value(), 8);

        let addr2 = layout.allocate_global();
        assert_eq!(addr2.value(), 9);

        assert_eq!(layout.global_size(), 2);
    }

    #[test]
    fn test_initial_local_heap() {
        let mut layout = MemoryLayout::new();
        layout.allocate_global();
        layout.allocate_global();

        let local_start = layout.initial_local_heap();
        assert_eq!(local_start.value(), 10); // 8 + 2
    }
}
