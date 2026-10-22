//! グローバル領域の配置

use crate::base::error::compile_error::{CompileError, CompileErrorKind};
use crate::compiler_ws::block_var_count::count_nested_vars_in_statements;
use crate::compiler_ws::memory::heap_layout;
use crate::semantic_analyzer::Program;
use std::ops::Range;

/// グローバル領域（GLOBAL_PTR からの相対オフセット）の配置
///
/// グローバル変数・static 変数・初期化文が使う変数の配置を、
/// コード生成の前に一度だけ計算する。
///
/// ```text
/// [0, V)              グローバル変数と、関数スコープ・ブロックスコープのすべての static 変数
///                     （V は Program::global_slot_count。意味解析がオフセットを割り当て済み）
/// [V, V+R)            ルート初期化文（static の初期化・ルートの文）のブロック変数
/// ```
#[derive(Debug)]
pub struct GlobalLayout {
    /// ルート初期化文（すべての static の初期化・ルートの文）のブロック変数領域 [begin, end)
    root_init_block_range: Range<i64>,
    /// グローバル領域の合計サイズ（スロット数）
    total_size: i64,
}

impl GlobalLayout {
    /// 意味解析の結果からグローバル領域の配置を計算する
    ///
    /// バグ: ルートレベルのブロック（`{ let: b; }`、`if`、初期化子のブロック）の変数が
    /// どこにも予約されておらず、オフセット 0 から積まれて
    /// グローバル変数・static 変数・アロケータの管理領域と同じアドレスに配置されていた。
    /// 解決策: これらの領域をグローバル領域の末尾に予約し、`total_size`（= global_heap_size）に含める。
    ///
    /// バグ: オフセットの累積が未チェックの `as i64` と `+` で、巨大な配列があると debug では panic、
    /// release では wrap して、グローバル領域が重なったコードを出力していた。
    /// 解決策: すべて checked にし、`GLOBAL_PTR + total_size + allocator_tail_slots` が WS のアドレス（i64）に
    /// 収まらない場合はコンパイルエラーにする。この検査により、領域内の個々のアドレス（`GLOBAL_PTR + offset`）と、
    /// アロケータがグローバル領域の直後に置く管理領域のアドレスも i64 に収まることが保証される
    /// （ルート初期化文の `enter_block_scope` もこの範囲内で動く）。
    ///
    /// 以前は関数内 static 変数の表と、関数ごとの static 初期化子のフレーム相当領域（StaticInitFrame）も
    /// ここで配置していた。static 変数は意味解析がグローバル領域に割り当て、初期化子はすべてルートの文脈で
    /// 実行するようになったので、どちらも不要になった（static-storage-unification）。
    ///
    /// `allocator_tail_slots`: アロケータがグローバル領域の直後に置く管理領域のスロット数
    /// （`AllocRuntime::reserved_tail_slots`）
    pub fn compute(program: &Program, allocator_tail_slots: i64) -> Result<Self, CompileError> {
        // グローバル変数・static 変数の直後
        let next_offset =
            to_i64(Some(program.global_slot_count)).ok_or_else(address_space_error)?;

        // ルート初期化文のブロック変数
        // static の初期化とルートの文は同じコンテキストで next_var_offset が通しで増えるため、
        // 1 つの領域にまとめる。ルートスコープ自身の変数はグローバル変数として配置済み。
        let root_block_count = count_nested_vars_in_statements(&program.static_init_statements)
            .zip(count_nested_vars_in_statements(&program.root_statements))
            .and_then(|(a, b)| a.checked_add(b));
        let root_init_block_range = next_offset
            ..checked_offset_add(next_offset, root_block_count).ok_or_else(address_space_error)?;
        let total_size = root_init_block_range.end;

        // GLOBAL_PTR からの絶対アドレスと、その直後のアロケータの管理領域も i64 に収まること
        // バグ: 管理領域の分を検査しておらず、FSBA の初期化（managed_start + i）で debug は panic、
        // release は wrap していた。
        // 解決策: allocator_tail_slots も含めて checked_add で検査する。
        heap_layout::GLOBAL_PTR
            .checked_add(total_size)
            .and_then(|end| end.checked_add(allocator_tail_slots))
            .ok_or_else(address_space_error)?;

        Ok(Self {
            root_init_block_range,
            total_size,
        })
    }

    /// ルート初期化文のブロック変数領域
    pub fn root_init_block_range(&self) -> Range<i64> {
        self.root_init_block_range.clone()
    }

    /// グローバル領域の合計サイズ（スロット数）
    pub fn total_size(&self) -> i64 {
        self.total_size
    }
}

fn address_space_error() -> CompileError {
    CompileError::new(CompileErrorKind::InvalidOperation(
        "total size of global and static variables exceeds the address space".to_string(),
    ))
}

/// スロット数（None は usize での合計の失敗）を i64 に変換する（収まらない場合は None）
fn to_i64(count: Option<usize>) -> Option<i64> {
    count.and_then(|n| i64::try_from(n).ok())
}

/// オフセットにスロット数を足す（i64 に収まらない場合は None）
fn checked_offset_add(offset: i64, count: Option<usize>) -> Option<i64> {
    offset.checked_add(to_i64(count)?)
}

#[cfg(test)]
#[path = "global_layout_tests.rs"]
mod tests;
