//! グローバル領域の配置

use crate::base::error::compile_error::{CompileError, CompileErrorKind};
use crate::compiler_ws::block_var_count::count_nested_vars_in_statements;
use crate::compiler_ws::memory::heap_layout;
use crate::semantic_analyzer::Scope;
use std::collections::HashMap;
use std::ops::Range;

/// 関数内 static 初期化子のフレーム相当領域
///
/// 関数フレームと同じ並びで、`[frame_base, block_base)` に関数スコープ直下の非 static 変数
/// （スロット番号をそのまま使う）、`[block_base, end)` に初期化子内のブロック変数を置く。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticInitFrame {
    /// 関数スコープ（scope_offsets[0]）のベースオフセット
    pub frame_base: i64,
    /// 初期化子内のブロック変数の開始オフセット
    pub block_base: i64,
    /// 領域の終端（この値を含まない）
    pub end: i64,
}

/// グローバル領域（GLOBAL_PTR からの相対オフセット）の配置
///
/// グローバル変数・関数内 static 変数・初期化文が使う変数の配置を、
/// コード生成の前に一度だけ計算する。
///
/// ```text
/// [0, V)              グローバル変数
/// [V, V+S)            関数内 static 変数
/// [V+S, V+S+R)        ルート初期化文のブロック変数
/// [V+S+R, ...)        関数ごとの static 初期化子のフレーム相当領域（関数順）
/// ```
#[derive(Debug)]
pub struct GlobalLayout {
    /// 関数内 static 変数のオフセット
    /// キー: (関数インデックス, スロットインデックス)
    static_var_offsets: HashMap<(usize, usize), i64>,
    /// ルート初期化文（ルートの static 初期化・ルートの文）のブロック変数領域 [begin, end)
    root_init_block_range: Range<i64>,
    /// 関数内 static 初期化子のフレーム相当領域
    /// キー: 関数インデックス（static 初期化子を持たない関数は含まない）
    static_init_frames: HashMap<usize, StaticInitFrame>,
    /// グローバル領域の合計サイズ（スロット数）
    total_size: i64,
}

impl GlobalLayout {
    /// Scope からグローバル領域の配置を計算する
    ///
    /// バグ: ルートレベルのブロック（`{ let: b; }`、`if`、初期化子のブロック）の変数と、
    /// 関数内 static 初期化子が使う変数（初期化子内のブロック変数・関数スコープ直下の非 static 変数）が
    /// どこにも予約されておらず、オフセット 0 / `func_scope_var_count` から積まれて
    /// グローバル変数・static 変数・アロケータの管理領域と同じアドレスに配置されていた。
    /// 解決策: これらの領域をグローバル領域の末尾に予約し、`total_size`（= global_heap_size）に含める。
    ///
    /// バグ: オフセットの累積が未チェックの `as i64` と `+` で、巨大な配列があると debug では panic、
    /// release では wrap して、グローバル領域が重なったコードを出力していた。
    /// 解決策: すべて checked にし、`GLOBAL_PTR + total_size + allocator_tail_slots` が WS のアドレス（i64）に
    /// 収まらない場合はコンパイルエラーにする。この検査により、領域内の個々のアドレス（`GLOBAL_PTR + offset`）と、
    /// アロケータがグローバル領域の直後に置く管理領域のアドレスも i64 に収まることが保証される
    /// （ルート・static 初期化子の `enter_block_scope` もこの範囲内で動く）。
    ///
    /// `allocator_tail_slots`: アロケータがグローバル領域の直後に置く管理領域のスロット数
    /// （`AllocRuntime::reserved_tail_slots`）
    pub fn compute(scope: &Scope, allocator_tail_slots: i64) -> Result<Self, CompileError> {
        // グローバル変数の直後
        let mut next_offset = to_i64(Some(scope.variable_count)).ok_or_else(address_space_error)?;

        // 関数内 static 変数
        let mut static_var_offsets = HashMap::new();
        for (func_idx, func) in scope.functions.iter().enumerate() {
            for var in &func.block.scope.variables {
                if var.is_static {
                    let slot_count = var.array_size.unwrap_or(1);
                    static_var_offsets.insert((func_idx, var.slot_index), next_offset);
                    next_offset = checked_offset_add(next_offset, Some(slot_count))
                        .ok_or_else(address_space_error)?;
                }
            }
        }

        // ルート初期化文のブロック変数
        // ルートの static 初期化とルートの文は同じコンテキストで next_var_offset が通しで増えるため、
        // 1 つの領域にまとめる。ルートスコープ自身の変数はグローバル変数として配置済み。
        let root_block_count = count_nested_vars_in_statements(&scope.static_init_statements)
            .zip(count_nested_vars_in_statements(&scope.root_statements))
            .and_then(|(a, b)| a.checked_add(b));
        let root_init_block_range = next_offset
            ..checked_offset_add(next_offset, root_block_count).ok_or_else(address_space_error)?;
        next_offset = root_init_block_range.end;

        // 関数内 static 初期化子のフレーム相当領域
        // 未使用関数も数える（static 変数の配置と揃える。生成されないので領域が余るだけ）
        let mut static_init_frames = HashMap::new();
        for (func_idx, func) in scope.functions.iter().enumerate() {
            let func_scope = &func.block.scope;
            if func_scope.static_init_statements.is_empty() {
                continue;
            }
            // 関数スコープ直下の非 static 変数（初期化子から参照されうる）は、関数フレームと同じ
            // スロット番号で置く。static 変数は static 領域に解決されるので、最後の非 static 変数
            // までを確保すればよい（static 配列の分を余分に確保しない）。
            // 初期化子が実際に参照するかは見ないので、初期化子の無い `static: s;` だけの関数でも
            // 引数・非 static 変数の分を確保する（アドレス空間が余るだけ）
            // slot_index + array_size は関数スコープの variable_count 以下（意味解析で usize に収まることを検査済み）
            let frame_var_count = func_scope
                .variables
                .iter()
                .filter(|var| !var.is_static)
                .map(|var| var.slot_index + var.array_size.unwrap_or(1))
                .max()
                .unwrap_or(0);
            let block_count = count_nested_vars_in_statements(&func_scope.static_init_statements);
            // 使われない関数でも、非 static 変数の分をグローバル領域に確保するため、
            // 巨大なローカル配列を持つ関数ではここで溢れうる。原因が分かるよう関数名を出す
            let frame_error = || {
                let func_name = scope
                    .symbol_table
                    .function_names
                    .get(func_idx)
                    .map(String::as_str)
                    .unwrap_or("?");
                CompileError::new(CompileErrorKind::InvalidOperation(format!(
                    "total size of global and static variables exceeds the address space \
                     (including the region for static initializers of function '{}', \
                     which reserves its non-static variables)",
                    func_name
                )))
            };
            let block_base =
                checked_offset_add(next_offset, Some(frame_var_count)).ok_or_else(frame_error)?;
            let frame = StaticInitFrame {
                frame_base: next_offset,
                block_base,
                end: checked_offset_add(block_base, block_count).ok_or_else(frame_error)?,
            };
            next_offset = frame.end;
            static_init_frames.insert(func_idx, frame);
        }

        // GLOBAL_PTR からの絶対アドレスと、その直後のアロケータの管理領域も i64 に収まること
        // バグ: 管理領域の分を検査しておらず、FSBA の初期化（managed_start + i）で debug は panic、
        // release は wrap していた。
        // 解決策: allocator_tail_slots も含めて checked_add で検査する。
        heap_layout::GLOBAL_PTR
            .checked_add(next_offset)
            .and_then(|end| end.checked_add(allocator_tail_slots))
            .ok_or_else(address_space_error)?;

        Ok(Self {
            static_var_offsets,
            root_init_block_range,
            static_init_frames,
            total_size: next_offset,
        })
    }

    /// 関数内 static 変数のオフセットを取得する（static でなければ None）
    pub fn static_var_offset(&self, func_index: usize, slot_index: usize) -> Option<i64> {
        self.static_var_offsets
            .get(&(func_index, slot_index))
            .copied()
    }

    /// ルート初期化文のブロック変数領域
    pub fn root_init_block_range(&self) -> Range<i64> {
        self.root_init_block_range.clone()
    }

    /// 関数内 static 初期化子のフレーム相当領域（static 初期化子を持たない関数は None）
    pub fn static_init_frame(&self, func_index: usize) -> Option<StaticInitFrame> {
        self.static_init_frames.get(&func_index).cloned()
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
