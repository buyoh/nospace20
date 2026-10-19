//! 関数内 static 変数の領域と、static 変数への参照の解決

use super::allocator::InterpreterAllocator;
use crate::semantic_analyzer::{IdentifierRef, Scope};

/// 関数内 static 変数の領域（関数ごとのベースアドレスと static スロットの表）
///
/// static 変数への参照を static 領域のアドレスへ直接解決する。NospaceVM とツリー走査インタプリタで共有する。
/// 解決規則は WS コンパイラの `CodeGenContext::get_var_info` と同じ。
///
/// バグ修正: 従来は関数呼び出し時に static 変数をフレームへコピーし、戻るときに書き戻していた
/// （コピーイン・コピーアウト）。再帰すると、外側の呼び出しが持つ古いコピーで内側の呼び出しの更新を
/// 上書きしていた。static 変数のアドレスもフレーム上のコピーを指していた。
/// 解決策: static 変数への参照を、この構造体で static 領域のアドレスへ解決する。
pub(crate) struct FunctionStaticStorage {
    /// 関数インデックス → static 領域（static 変数を持たない関数は None）
    regions: Vec<Option<StaticRegion>>,
}

/// 1 関数分の static 領域
struct StaticRegion {
    /// static 領域のベースアドレス（アロケータ上。関数スコープの variable_count 分）
    base_addr: i64,
    /// 関数スコープのスロットごとに、static 変数のスロットか（配列は全要素のスロットを true）
    static_slots: Vec<bool>,
}

/// 実行中の関数呼び出し
#[derive(Clone, Copy, Debug)]
pub(crate) struct FuncFrame {
    pub func_idx: usize,
    /// 関数スコープの領域が積まれている `scope_stack` 上のインデックス
    pub scope_index: usize,
}

impl FunctionStaticStorage {
    pub fn new() -> Self {
        FunctionStaticStorage {
            regions: Vec::new(),
        }
    }

    /// static 変数を持つ全関数の static 領域を確保する（初期化子は実行しない）
    pub fn allocate(
        scope: &Scope,
        allocator: &mut InterpreterAllocator,
        randomize_uninit: bool,
    ) -> Self {
        let regions = scope
            .functions
            .iter()
            .map(|func| {
                let func_scope = &func.block.scope;
                if !func_scope.variables.iter().any(|v| v.is_static) {
                    return None;
                }
                let base_addr =
                    allocator.alloc_internal_uninit(func_scope.variable_count, randomize_uninit);
                let mut static_slots = vec![false; func_scope.variable_count];
                for v in func_scope.variables.iter().filter(|v| v.is_static) {
                    let end = v.slot_index + v.array_size.unwrap_or(1);
                    // 意味解析が variable_count をスロットの総数として数えている（内部の不変条件）
                    debug_assert!(end <= static_slots.len());
                    for slot in v.slot_index..end.min(static_slots.len()) {
                        static_slots[slot] = true;
                    }
                }
                Some(StaticRegion {
                    base_addr,
                    static_slots,
                })
            })
            .collect();
        FunctionStaticStorage { regions }
    }

    pub fn base_addr(&self, func_idx: usize) -> Option<i64> {
        self.region(func_idx).map(|r| r.base_addr)
    }

    /// static 変数への参照なら static 領域のアドレスを返す
    ///
    /// - `owning_func_index: Some(owner)`（ネストした関数から親関数の static）: `owner` の領域
    /// - 参照先が実行中の関数（`current`）の関数スコープで、そのスロットが static: `current` の領域
    ///
    /// 該当しない場合（グローバル変数、非 static 変数、static 領域が無い場合）は None を返し、
    /// 呼び出し側が `scope_stack` で解決する。
    pub fn resolve(
        &self,
        id: &IdentifierRef,
        current: Option<FuncFrame>,
        scope_stack_len: usize,
    ) -> Option<i64> {
        if id.is_global {
            return None;
        }
        if let Some(owner) = id.owning_func_index {
            return self
                .base_addr(owner)
                .map(|base| base + id.local_index as i64);
        }
        let frame = current?;
        let target = scope_stack_len.checked_sub(1 + id.scope_depth)?;
        if target != frame.scope_index {
            return None;
        }
        let region = self.region(frame.func_idx)?;
        if region
            .static_slots
            .get(id.local_index)
            .copied()
            .unwrap_or(false)
        {
            Some(region.base_addr + id.local_index as i64)
        } else {
            None
        }
    }

    fn region(&self, func_idx: usize) -> Option<&StaticRegion> {
        self.regions.get(func_idx).and_then(|r| r.as_ref())
    }
}

#[cfg(test)]
#[path = "static_storage_tests.rs"]
mod tests;
