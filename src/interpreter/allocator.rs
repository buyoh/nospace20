//! インタプリタ用メモリアロケータ
//!
//! WS コンパイラと同一の FSBA + First-Fit + バンプ アルゴリズムを Rust で直接実行する。
//! アルゴリズム定数は `crate::algorithm::alloc_spec` から参照する。
//!
//! ## アドレスモデル
//!
//! FSBA/汎用アロケータ（`alloc`/`free` 用）と内部アロケータ（`alloc_internal`/`free_internal` 用）の
//! 2 種類の確保方式を持つ。
//!
//! - `alloc(user_size)` → ヘッダー付きブロックを確保し `ptr = block + 1` を返す
//! - `alloc_internal(size)` → ヘッダーなしブロックを確保しブロック開始アドレスをそのまま返す

use std::collections::BTreeMap;

use crate::algorithm::alloc_spec;

// ===== データ構造 =====

/// 単一のメモリブロック
struct MemoryBlock {
    /// ブロックのデータ（実際のメモリ）
    ///
    /// FSBA/汎用ブロック: data[0] = total_size (ヘッダー), data[1..] = ユーザーデータ
    /// 内部ブロック: data[0..] = ユーザーデータ（ヘッダーなし）
    data: Vec<i64>,
    /// 解放済みフラグ（`is_freed=true` でも `blocks` に残る）
    is_freed: bool,
}

/// インタプリタ用メモリアロケータ
///
/// WS コンパイラと同一の FSBA + First-Fit + バンプ アルゴリズムを
/// Rust で直接実行する。アルゴリズム定数は `alloc_spec` から参照。
///
/// 仮想1次元アドレス空間を管理する。
/// 各アロケーションは独立した `Vec<i64>` で保持され、
/// `BTreeMap` で仮想アドレスから実際のブロックへマッピングされる。
pub(crate) struct InterpreterAllocator {
    /// ブロック開始アドレス → メモリブロック のマッピング
    blocks: BTreeMap<i64, MemoryBlock>,
    /// 次に割り当てる仮想アドレス（バンプポインタ）
    next_addr: i64,
    /// FSBA サイズクラスごとのフリーリスト先頭ブロックアドレス (0 = 空)
    fsba_free_lists: [i64; alloc_spec::FSBA_CLASS_COUNT],
    /// 汎用フリーリスト先頭ブロックアドレス (0 = 空)
    general_free_head: i64,
}

impl InterpreterAllocator {
    /// 新しいアロケータを作成する。
    ///
    /// アドレス 0 はフリーリストの「空」を表すセンチネル値として使用するため、
    /// 実際の割り当ては 1 から始まる。
    pub(crate) fn new() -> Self {
        InterpreterAllocator {
            blocks: BTreeMap::new(),
            next_addr: 1, // 0 はフリーリストのセンチネル値（空を表す）
            fsba_free_lists: [0; alloc_spec::FSBA_CLASS_COUNT],
            general_free_head: 0,
        }
    }

    // ===== 公開 API: FSBA / First-Fit アロケータ =====

    /// ユーザーサイズ分のメモリを確保し、ユーザーポインタ（ptr = block + 1）を返す。
    ///
    /// FSBA → 汎用 First-Fit → バンプ の順でフォールバック。
    /// 返したアドレスは `free(ptr)` で解放できる。
    pub(crate) fn alloc(&mut self, user_size: i64) -> i64 {
        let total = alloc_spec::total_from_user_size(user_size);
        match alloc_spec::fsba_class_for(total) {
            Some(class) => self.fsba_alloc(class),
            None => self.general_alloc(total),
        }
    }

    /// 0 または ランダム値で初期化してメモリを確保する。
    ///
    /// `randomize=false`: 0 初期化（通常モード）
    /// `randomize=true`: 擬似ランダム値で初期化（未初期化変数バグ検出用）
    pub(crate) fn alloc_uninit(&mut self, user_size: i64, randomize: bool) -> i64 {
        let ptr = self.alloc(user_size);
        if randomize {
            let block_addr = ptr - 1;
            let block = self.blocks.get_mut(&block_addr).unwrap();
            // data[0] はヘッダー（total_size）なので変更しない
            let len = block.data.len();
            for i in 1..len {
                block.data[i] = uninit_value(block_addr, i);
            }
        }
        ptr
    }

    /// 指定ポインタが指すメモリブロックを解放する。
    ///
    /// - 存在しないアドレス: `panic!`
    /// - 二重 free: `panic!`
    /// - ブロック先頭以外の free: `panic!`
    pub(crate) fn free(&mut self, ptr: i64) {
        let block_addr = ptr - 1;

        // ブロック先頭かチェック（ptr-1 がブロック開始でない場合）
        if !self.blocks.contains_key(&block_addr) {
            // ptr-1 がブロック開始でない場合、より詳しいエラーメッセージを出す
            if let Some((block_start, _)) = self.find_block_containing(block_addr) {
                panic!(
                    "runtime error: free: address {} is not a block start (block starts at {})",
                    ptr,
                    block_start + 1
                );
            }
            panic!("runtime error: free: invalid address {}", ptr);
        }

        let block = self.blocks.get_mut(&block_addr).unwrap();
        if block.is_freed {
            panic!("runtime error: double free at address {}", ptr);
        }

        let block_size = block.data[0];
        block.is_freed = true;

        match alloc_spec::fsba_class_for(block_size) {
            Some(class) => self.fsba_free(block_addr, class),
            None => self.general_free(block_addr),
        }
    }

    /// 指定アドレスの値を読み取る。
    ///
    /// - 未割当アドレス: `panic!`
    /// - 解放済みアドレス: `panic!`
    pub(crate) fn get(&self, addr: i64) -> i64 {
        let (block_start, block) = self
            .find_block_containing(addr)
            .unwrap_or_else(|| panic!("runtime error: invalid memory access at address {}", addr));
        if block.is_freed {
            panic!("runtime error: access to freed memory at address {}", addr);
        }
        let offset = (addr - block_start) as usize;
        block.data[offset]
    }

    /// 指定アドレスに値を書き込む。
    ///
    /// - 未割当アドレス: `panic!`
    /// - 解放済みアドレス: `panic!`
    pub(crate) fn set(&mut self, addr: i64, value: i64) {
        let (block_start, block) = self
            .find_block_containing_mut(addr)
            .unwrap_or_else(|| panic!("runtime error: invalid memory access at address {}", addr));
        if block.is_freed {
            panic!("runtime error: access to freed memory at address {}", addr);
        }
        let offset = (addr - block_start) as usize;
        block.data[offset] = value;
    }

    // ===== 公開 API: 内部アロケータ（スコープ・グローバル変数用） =====

    /// ヘッダーなしでサイズ分のメモリを確保し、ブロック開始アドレスをそのまま返す。
    ///
    /// FSBA/汎用アロケータとは별도に管理される。
    /// 返したアドレスは `free_internal(addr)` で解放できる。
    /// LIFO パターンで解放されることを前提とする（フリーリストなし）。
    pub(crate) fn alloc_internal(&mut self, size: usize) -> i64 {
        let size = if size == 0 { 1 } else { size };

        let addr = self.next_addr;
        self.next_addr += size as i64;

        let block = MemoryBlock {
            data: vec![0i64; size],
            is_freed: false,
        };
        self.blocks.insert(addr, block);

        addr
    }

    /// 未初期化（0 or ランダム値）で内部メモリを確保する。
    pub(crate) fn alloc_internal_uninit(&mut self, size: usize, randomize: bool) -> i64 {
        let addr = self.alloc_internal(size);
        if randomize {
            let block = self.blocks.get_mut(&addr).unwrap();
            let len = block.data.len();
            for i in 0..len {
                block.data[i] = uninit_value(addr, i);
            }
        }
        addr
    }

    /// 内部アロケータで確保したブロックを解放する。
    ///
    /// `is_freed = true` にするのみ（フリーリストに返さない）。
    pub(crate) fn free_internal(&mut self, addr: i64) {
        if let Some(block) = self.blocks.get_mut(&addr) {
            block.is_freed = true;
        }
        // 存在しないアドレスは静かに無視（スコープ管理の都合上）
    }

    // ===== プライベートヘルパー =====

    /// FSBA からメモリを確保する（フリーリスト → バンプ）。
    fn fsba_alloc(&mut self, class: usize) -> i64 {
        let class_size = alloc_spec::FSBA_BLOCK_SIZES[class];
        let free_head = self.fsba_free_lists[class];

        if free_head != 0 {
            // フリーリストからポップ
            let next = self.blocks[&free_head].data[1];
            let block = self.blocks.get_mut(&free_head).unwrap();
            block.is_freed = false;
            self.fsba_free_lists[class] = next;
            return free_head + 1;
        }

        // フリーリスト空 → バンプ割り当て
        self.bump_alloc(class_size)
    }

    /// 汎用 First-Fit アロケータ（フリーリスト探索 → バンプ）。
    fn general_alloc(&mut self, total: i64) -> i64 {
        let mut prev_block_addr: Option<i64> = None; // None = head 自体
        let mut curr = self.general_free_head;

        while curr != 0 {
            let curr_size = self.blocks[&curr].data[0];
            let next = self.blocks[&curr].data[1];

            if curr_size >= total {
                // ブロック発見
                if alloc_spec::can_split(curr_size, total) {
                    // 分割: 前半を使用、後半をフリーリストに残す
                    let remainder_addr = curr + total;
                    let remainder_size = curr_size - total;

                    // 残余ブロックを作成
                    let mut rem_data = vec![0i64; remainder_size as usize];
                    rem_data[0] = remainder_size;
                    if remainder_size > 1 {
                        rem_data[1] = next; // next pointer 継承
                    }
                    self.blocks.insert(
                        remainder_addr,
                        MemoryBlock {
                            data: rem_data,
                            is_freed: false,
                        },
                    );

                    // 現ブロックを縮小
                    {
                        let block = self.blocks.get_mut(&curr).unwrap();
                        block.data[0] = total;
                        block.data.truncate(total as usize);
                        block.is_freed = false;
                    }

                    // prev の next を remainder に更新
                    self.set_prev_next(prev_block_addr, remainder_addr);
                    return curr + 1;
                } else {
                    // 分割不可: ブロック全体を使用
                    {
                        let block = self.blocks.get_mut(&curr).unwrap();
                        block.is_freed = false;
                    }
                    self.set_prev_next(prev_block_addr, next);
                    return curr + 1;
                }
            }

            // 次のブロックへ
            prev_block_addr = Some(curr);
            curr = next;
        }

        // 適合ブロックなし → バンプ割り当て
        self.bump_alloc(total)
    }

    /// バンプポインタで新しいブロックを割り当てる。
    fn bump_alloc(&mut self, total: i64) -> i64 {
        let addr = self.next_addr;
        self.next_addr += total;

        let mut data = vec![0i64; total as usize];
        data[0] = total; // ヘッダー: ブロック合計サイズ
        self.blocks.insert(
            addr,
            MemoryBlock {
                data,
                is_freed: false,
            },
        );

        addr + 1 // ptr = block + 1
    }

    /// FSBA フリーリストにブロックを返却する。
    fn fsba_free(&mut self, block_addr: i64, class: usize) {
        let old_head = self.fsba_free_lists[class];
        let block = self.blocks.get_mut(&block_addr).unwrap();
        if block.data.len() > 1 {
            block.data[1] = old_head; // next = old head
        }
        self.fsba_free_lists[class] = block_addr;
    }

    /// 汎用フリーリストにブロックを返却する。
    fn general_free(&mut self, block_addr: i64) {
        let old_head = self.general_free_head;
        let block = self.blocks.get_mut(&block_addr).unwrap();
        if block.data.len() > 1 {
            block.data[1] = old_head; // next = old head
        }
        self.general_free_head = block_addr;
    }

    /// First-Fit ループで使う「前のノードの next ポインタ」更新ヘルパー。
    ///
    /// `prev=None` の場合は `general_free_head` を更新する。
    fn set_prev_next(&mut self, prev_block_addr: Option<i64>, new_next: i64) {
        match prev_block_addr {
            None => self.general_free_head = new_next,
            Some(p) => {
                let block = self.blocks.get_mut(&p).unwrap();
                block.data[1] = new_next;
            }
        }
    }

    /// `addr` を含むブロックを返す（不変参照）。
    ///
    /// `BTreeMap::range(..=addr).next_back()` で O(log n) 検索。
    fn find_block_containing(&self, addr: i64) -> Option<(i64, &MemoryBlock)> {
        let (block_start, block) = self.blocks.range(..=addr).next_back()?;
        let block_start = *block_start;
        // アドレスがブロック範囲内かチェック
        if addr >= block_start + block.data.len() as i64 {
            return None;
        }
        Some((block_start, block))
    }

    /// `addr` を含むブロックを返す（可変参照）。
    fn find_block_containing_mut(&mut self, addr: i64) -> Option<(i64, &mut MemoryBlock)> {
        // まずブロック開始アドレスを確定してから可変参照を取る
        let block_start = {
            let (bs, block) = self.blocks.range(..=addr).next_back()?;
            let bs = *bs;
            if addr >= bs + block.data.len() as i64 {
                return None;
            }
            bs
        };
        let block = self.blocks.get_mut(&block_start).unwrap();
        Some((block_start, block))
    }
}

/// 未初期化値として使う決定論的な擬似ランダム値
///
/// デバッグ再現性のため、アドレスとオフセットから決定論的に生成する。
fn uninit_value(addr: i64, offset: usize) -> i64 {
    // 単純な線形合同法ベースのハッシュ（デバッグ再現性のため固定シード）
    let seed = addr.wrapping_mul(6364136223846793005)
        ^ (offset as i64).wrapping_mul(2891336453);
    seed.wrapping_add(1442695040888963407)
}

// ===== ユニットテスト =====

#[cfg(test)]
#[path = "allocator_tests.rs"]
mod tests;
