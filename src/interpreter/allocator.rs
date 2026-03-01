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
mod tests {
    use super::*;

    fn new_alloc() -> InterpreterAllocator {
        InterpreterAllocator::new()
    }

    // --- 基本 alloc / get / set ---

    #[test]
    fn test_alloc_basic() {
        let mut a = new_alloc();
        let ptr = a.alloc(3);
        // ptr は 1 以上の正のアドレス
        assert!(ptr > 0);
        // get は 0 初期化
        assert_eq!(a.get(ptr), 0);
        assert_eq!(a.get(ptr + 1), 0);
        assert_eq!(a.get(ptr + 2), 0);
        // set / get が正常動作
        a.set(ptr, 42);
        assert_eq!(a.get(ptr), 42);
        a.set(ptr + 2, 99);
        assert_eq!(a.get(ptr + 2), 99);
    }

    #[test]
    fn test_alloc_multiple() {
        let mut a = new_alloc();
        let p1 = a.alloc(3);
        let p2 = a.alloc(3);
        let p3 = a.alloc(5);
        // 各ポインタは互いに異なる
        assert_ne!(p1, p2);
        assert_ne!(p2, p3);
        // 各ブロックの書き込みが独立している
        a.set(p1, 10);
        a.set(p2, 20);
        a.set(p3, 30);
        assert_eq!(a.get(p1), 10);
        assert_eq!(a.get(p2), 20);
        assert_eq!(a.get(p3), 30);
    }

    #[test]
    fn test_alloc_zero_size() {
        let mut a = new_alloc();
        // alloc(0) は alloc(1) と同等: 最低でも 1 つの要素にアクセス可能
        let p0 = a.alloc(0);
        let p1 = a.alloc(1);
        // 両方正常にアクセスできる
        a.set(p0, 7);
        a.set(p1, 8);
        assert_eq!(a.get(p0), 7);
        assert_eq!(a.get(p1), 8);
        // アドレスが重ならない
        assert_ne!(p0, p1);
    }

    // --- free ---

    #[test]
    fn test_free_basic() {
        let mut a = new_alloc();
        let ptr = a.alloc(3);
        a.set(ptr, 42);
        a.free(ptr);
        // 解放後に get すると panic
        let result = std::panic::catch_unwind(move || a.get(ptr));
        assert!(result.is_err());
    }

    #[test]
    fn test_free_invalid_address() {
        let mut a = new_alloc();
        // 存在しないアドレスの free は panic
        let result = std::panic::catch_unwind(move || a.free(99999));
        assert!(result.is_err());
    }

    #[test]
    fn test_double_free() {
        let mut a = new_alloc();
        let ptr = a.alloc(4);
        a.free(ptr);
        // 二重 free は panic
        let result = std::panic::catch_unwind(move || a.free(ptr));
        assert!(result.is_err());
    }

    // --- アクセスエラー ---

    #[test]
    fn test_access_unallocated() {
        let a = new_alloc();
        // 未割当アドレスへのアクセスは panic
        let result = std::panic::catch_unwind(move || a.get(99999));
        assert!(result.is_err());
    }

    #[test]
    fn test_access_freed() {
        let mut a = new_alloc();
        let ptr = a.alloc(3);
        a.free(ptr);
        // 解放済みアドレスへのアクセスは panic
        let result = std::panic::catch_unwind(move || a.get(ptr));
        assert!(result.is_err());
    }

    #[test]
    fn test_block_boundary() {
        let mut a = new_alloc();
        let ptr = a.alloc(3); // ユーザーサイズ 3 → ptr, ptr+1, ptr+2 がアクセス可能
        // ptr+3 はブロック境界外
        let result = std::panic::catch_unwind(move || a.get(ptr + 3));
        assert!(result.is_err());
    }

    // --- alloc_uninit ---

    #[test]
    fn test_alloc_uninit_zero() {
        let mut a = new_alloc();
        let ptr = a.alloc_uninit(4, false);
        // 0 初期化
        assert_eq!(a.get(ptr), 0);
        assert_eq!(a.get(ptr + 1), 0);
        assert_eq!(a.get(ptr + 2), 0);
        assert_eq!(a.get(ptr + 3), 0);
    }

    #[test]
    fn test_alloc_uninit_random() {
        let mut a = new_alloc();
        let ptr = a.alloc_uninit(4, true);
        // ランダムモードでは少なくとも 1 つは 0 以外の値（実装では決定論的に非 0 を保証しないが、
        // ほとんどのケースで非 0 になる）
        // ここでは panic しないことと get できることを確認
        let _ = a.get(ptr);
        let _ = a.get(ptr + 3);
    }

    // --- FSBA フリーリスト再利用 ---

    #[test]
    fn test_fsba_free_reuse() {
        let mut a = new_alloc();
        // FSBA クラス 0 (block_size=2, user_size=1)
        let p1 = a.alloc(1);
        let p2 = a.alloc(1);
        a.set(p1, 111);
        a.set(p2, 222);

        // p1 を解放してから再度 alloc → p1 が再利用される
        a.free(p1);
        let p3 = a.alloc(1);
        // p3 は p1 と同じアドレスのはず（FSBA フリーリストから再利用）
        assert_eq!(p3, p1);
        // p2 は変わらない
        assert_eq!(a.get(p2), 222);
    }

    #[test]
    fn test_general_alloc_first_fit() {
        let mut a = new_alloc();
        // general alloc (user_size > 31: total > 32)
        let p1 = a.alloc(50);
        let _p2 = a.alloc(50);
        a.free(p1);
        // First-Fit: p1 のブロックが再利用される
        let p3 = a.alloc(40);
        assert_eq!(p3, p1);
    }

    // --- alloc_internal ---

    #[test]
    fn test_alloc_internal_basic() {
        let mut a = new_alloc();
        let addr = a.alloc_internal(3);
        // ヘッダーなし: addr から直接アクセス
        assert_eq!(a.get(addr), 0);
        assert_eq!(a.get(addr + 1), 0);
        assert_eq!(a.get(addr + 2), 0);
        a.set(addr + 1, 55);
        assert_eq!(a.get(addr + 1), 55);
    }

    #[test]
    fn test_alloc_internal_free_internal() {
        let mut a = new_alloc();
        let addr = a.alloc_internal(4);
        a.set(addr, 10);
        a.free_internal(addr);
        // 解放後は get が panic
        let result = std::panic::catch_unwind(move || a.get(addr));
        assert!(result.is_err());
    }

    #[test]
    fn test_alloc_internal_uninit_random() {
        let mut a = new_alloc();
        let addr = a.alloc_internal_uninit(4, true);
        // panic しないことを確認
        let _ = a.get(addr);
        let _ = a.get(addr + 3);
    }

    // --- エラーメッセージ確認 ---

    #[test]
    fn test_free_invalid_address_message() {
        let mut a = new_alloc();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.free(99999)));
        let err = result.unwrap_err();
        let msg = err.downcast_ref::<String>().map(|s| s.as_str())
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(msg.contains("free"), "expected 'free' in error, got: {msg}");
    }

    #[test]
    fn test_double_free_message() {
        let mut a = new_alloc();
        let ptr = a.alloc(3);
        a.free(ptr);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.free(ptr)));
        let err = result.unwrap_err();
        let msg = err.downcast_ref::<String>().map(|s| s.as_str())
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(msg.contains("double free"), "expected 'double free' in error, got: {msg}");
    }

    #[test]
    fn test_access_freed_message() {
        let mut a = new_alloc();
        let ptr = a.alloc(3);
        a.free(ptr);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.get(ptr)));
        let err = result.unwrap_err();
        let msg = err.downcast_ref::<String>().map(|s| s.as_str())
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(msg.contains("freed memory"), "expected 'freed memory' in error, got: {msg}");
    }

    #[test]
    fn test_access_unallocated_message() {
        let a = new_alloc();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.get(99999)));
        let err = result.unwrap_err();
        let msg = err.downcast_ref::<String>().map(|s| s.as_str())
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            msg.contains("invalid memory access"),
            "expected 'invalid memory access' in error, got: {msg}"
        );
    }
}
