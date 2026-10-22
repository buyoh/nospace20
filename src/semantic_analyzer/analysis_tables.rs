//! # AnalysisTables
//!
//! 意味解析の全スコープ（ルート・関数・ブロック・式の中のブロック）で共有する表。

use super::function_table::FunctionTable;
use super::types::LocatedExecStatement;

/// 意味解析の全スコープで共有する表
///
/// static 変数の宣言は式の中のブロック（初期化子の中のブロックを含む）にも現れるので、
/// グローバル領域の割り当てと初期化文の収集は、関数表と同じく全経路で共有する。
pub(super) struct AnalysisTables {
    pub functions: FunctionTable,
    pub global_slots: GlobalSlotAllocator,
    /// すべての static 変数の初期化文（宣言の変換が終わった順 = ソース順。
    /// 初期化子の中で宣言した static は、その static より先に積まれる）
    pub static_init_statements: Vec<LocatedExecStatement>,
}

impl AnalysisTables {
    pub(super) fn new() -> Self {
        Self {
            functions: FunctionTable::new(),
            global_slots: GlobalSlotAllocator::new(),
            static_init_statements: Vec::new(),
        }
    }
}

/// グローバル領域の割り当て
///
/// ルートの変数（`[0, root_variable_count)`）の直後から、関数スコープ・ブロックスコープの
/// static 変数にオフセットを宣言順に割り当てる。
pub(super) struct GlobalSlotAllocator {
    next_slot: usize,
    /// `start` を呼んだか（ルートのスロット配置の前に割り当てると、ルートの変数と重なるため）
    started: bool,
}

impl GlobalSlotAllocator {
    pub(super) fn new() -> Self {
        Self {
            next_slot: 0,
            started: false,
        }
    }

    /// ルートのスロット配置（compute_slot_layout）の直後に 1 回だけ呼ぶ
    pub(super) fn start(&mut self, root_variable_count: usize) {
        debug_assert!(
            !self.started && self.next_slot == 0,
            "global slots must not be allocated before the root slot layout"
        );
        self.next_slot = root_variable_count;
        self.started = true;
    }

    /// `slot_count` スロットを割り当て、先頭のオフセットを返す。usize に収まらなければ None
    pub(super) fn allocate(&mut self, slot_count: usize) -> Option<usize> {
        debug_assert!(self.started, "global slots must be allocated after start");
        let offset = self.next_slot;
        self.next_slot = offset.checked_add(slot_count)?;
        Some(offset)
    }

    /// グローバル領域の大きさ（ルートの Scope::variable_count に設定する）
    pub(super) fn total(&self) -> usize {
        self.next_slot
    }
}
