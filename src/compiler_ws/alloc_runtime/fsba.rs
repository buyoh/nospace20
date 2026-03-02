//! FSBA + First-Fit アロケータ（FsbaFirstFitAllocRuntime）の実装

use crate::algorithm::alloc_spec;
use crate::compiler_ws::{
    instruction::Instruction, label::reserved_labels, memory::heap_layout, program::WsProgram,
    types::WsNumber,
};

use super::{AllocRuntime, generate_common_epilogue, generate_common_prologue};

// ===== FSBA + First-Fit アロケータ =====

/// FSBA 内部ラベル定義
///
/// 予約ラベル 16-47 の範囲を使用する。
/// これらは `FsbaFirstFitAllocRuntime::generate_subroutines()` 内でのみ使用される。
mod fsba_labels {
    use crate::compiler_ws::types::LabelId;

    // __rt_alloc 内部ラベル
    /// total < 2 の場合の最小値設定
    pub const ALLOC_SET_MIN: LabelId = LabelId(16);
    /// カスケード比較開始点
    pub const ALLOC_CASCADE: LabelId = LabelId(17);
    /// サイズクラス 0 (ブロック 2 セル) へのディスパッチ
    pub const ALLOC_CLASS_0: LabelId = LabelId(18);
    /// サイズクラス 1 (ブロック 4 セル)
    pub const ALLOC_CLASS_1: LabelId = LabelId(19);
    /// サイズクラス 2 (ブロック 8 セル)
    pub const ALLOC_CLASS_2: LabelId = LabelId(20);
    /// サイズクラス 3 (ブロック 16 セル)
    pub const ALLOC_CLASS_3: LabelId = LabelId(21);
    /// サイズクラス 4 (ブロック 32 セル)
    pub const ALLOC_CLASS_4: LabelId = LabelId(22);
    /// FSBA alloc 共通パス
    pub const FSBA_ALLOC_COMMON: LabelId = LabelId(23);
    /// FSBA バンプ拡張パス
    pub const FSBA_ALLOC_BUMP: LabelId = LabelId(24);
    /// 汎用アロケータ (First-Fit) エントリ
    pub const GENERAL_ALLOC: LabelId = LabelId(25);
    /// First-Fit ループ開始
    pub const GENERAL_ALLOC_LOOP: LabelId = LabelId(26);
    /// ループ: 次のフリーブロックへ
    pub const GENERAL_ALLOC_NEXT: LabelId = LabelId(27);
    /// ブロック発見
    pub const GENERAL_ALLOC_FOUND: LabelId = LabelId(28);
    /// ブロック分割なし
    pub const GENERAL_ALLOC_NO_SPLIT: LabelId = LabelId(29);
    /// 汎用バンプ拡張
    pub const GENERAL_ALLOC_BUMP: LabelId = LabelId(30);

    // __rt_free 内部ラベル
    /// free: サイズクラス 0
    pub const FREE_CLASS_0: LabelId = LabelId(31);
    /// free: サイズクラス 1
    pub const FREE_CLASS_1: LabelId = LabelId(32);
    /// free: サイズクラス 2
    pub const FREE_CLASS_2: LabelId = LabelId(33);
    /// free: サイズクラス 3
    pub const FREE_CLASS_3: LabelId = LabelId(34);
    /// free: サイズクラス 4
    pub const FREE_CLASS_4: LabelId = LabelId(35);
    /// FSBA free 共通パス
    pub const FSBA_FREE_COMMON: LabelId = LabelId(36);
    /// 汎用 free
    pub const GENERAL_FREE: LabelId = LabelId(37);
}

/// FSBA + First-Fit アロケータ
///
/// 二層アーキテクチャ:
/// - 第1層: FSBA (Fixed-Size Block Allocator) — サイズ ≤32 セルの高速割り当て
///   サイズクラス: 2, 4, 8, 16, 32 セル (ヘッダー含む)
/// - 第2層: 汎用 First-Fit + バンプ — サイズ >32 セルのフォールバック
///
/// `--std-ext alloc` 有効時に使用される。
pub struct FsbaFirstFitAllocRuntime;

/// FSBA サイズクラス定義: (class_index, block_size, cascade_threshold)
///
/// cascade_threshold は `total - threshold < 0` で該当クラスを判定するための値。
/// threshold = block_size + 1 とすることで `total <= block_size` を `total - (block_size+1) < 0` で判定する。
/// `block_size` は `alloc_spec::FSBA_BLOCK_SIZES` から導出する。
const FSBA_SIZE_CLASSES: [(i64, i64, i64); alloc_spec::FSBA_CLASS_COUNT] = {
    let bs = alloc_spec::FSBA_BLOCK_SIZES;
    [
        (0, bs[0], bs[0] + 1), // class 0: block_size=2,  threshold=3  → total <= 2
        (1, bs[1], bs[1] + 1), // class 1: block_size=4,  threshold=5  → total <= 4
        (2, bs[2], bs[2] + 1), // class 2: block_size=8,  threshold=9  → total <= 8
        (3, bs[3], bs[3] + 1), // class 3: block_size=16, threshold=17 → total <= 16
        (4, bs[4], bs[4] + 1), // class 4: block_size=32, threshold=33 → total <= 32
    ]
};

/// alloc カスケードのクラスラベル配列
const ALLOC_CLASS_LABELS: [crate::compiler_ws::types::LabelId; 5] = [
    fsba_labels::ALLOC_CLASS_0,
    fsba_labels::ALLOC_CLASS_1,
    fsba_labels::ALLOC_CLASS_2,
    fsba_labels::ALLOC_CLASS_3,
    fsba_labels::ALLOC_CLASS_4,
];

/// free カスケードのクラスラベル配列
const FREE_CLASS_LABELS: [crate::compiler_ws::types::LabelId; 5] = [
    fsba_labels::FREE_CLASS_0,
    fsba_labels::FREE_CLASS_1,
    fsba_labels::FREE_CLASS_2,
    fsba_labels::FREE_CLASS_3,
    fsba_labels::FREE_CLASS_4,
];

impl FsbaFirstFitAllocRuntime {
    /// __rt_alloc サブルーチンを生成
    fn generate_rt_alloc(&self) -> WsProgram {
        let mut prog = WsProgram::new();

        // === __rt_alloc(size) → ptr ===
        // スタック入力: [size]
        // スタック出力: [ptr]
        prog.push(Instruction::Label(reserved_labels::RT_ALLOC));

        // Step 1: total = max(size + 1, 2)
        prog.extend([
            // スタック: [size]
            Instruction::Push(WsNumber(alloc_spec::HEADER_SIZE)),
            Instruction::Add,
            // スタック: [size+1]
            Instruction::Duplicate,
            // スタック: [size+1, size+1]
            Instruction::Push(WsNumber(alloc_spec::MIN_BLOCK_SIZE)),
            Instruction::Sub,
            // スタック: [size+1, size+1-2]
            Instruction::JumpIfNegative(fsba_labels::ALLOC_SET_MIN),
            // size+1 >= 2: total = size+1
            Instruction::Jump(fsba_labels::ALLOC_CASCADE),
        ]);

        // ALLOC_SET_MIN: total < MIN_BLOCK_SIZE → total = MIN_BLOCK_SIZE
        prog.extend([
            Instruction::Label(fsba_labels::ALLOC_SET_MIN),
            // jn popped size+1-2. Stack: [size+1]
            Instruction::Discard,
            Instruction::Push(WsNumber(alloc_spec::MIN_BLOCK_SIZE)),
            // fall through to ALLOC_CASCADE
        ]);

        // ALLOC_CASCADE: サイズクラスカスケード比較
        prog.push(Instruction::Label(fsba_labels::ALLOC_CASCADE));
        // スタック: [total] where total >= 2

        // 各クラスについて: dup; push threshold; sub; jn CLASS_N
        // jn: total - threshold < 0 → total <= block_size → 該当クラス
        for (i, (_class_index, _block_size, threshold)) in FSBA_SIZE_CLASSES.iter().enumerate() {
            prog.extend([
                Instruction::Duplicate,
                Instruction::Push(WsNumber(*threshold)),
                Instruction::Sub,
                Instruction::JumpIfNegative(ALLOC_CLASS_LABELS[i]),
            ]);
        }

        // total > 32 → 汎用アロケータ
        prog.push(Instruction::Jump(fsba_labels::GENERAL_ALLOC));

        // === クラスハンドラ: total を破棄し、class_index と class_size を設定 ===
        for (class_index, block_size, _threshold) in &FSBA_SIZE_CLASSES {
            prog.extend([
                Instruction::Label(ALLOC_CLASS_LABELS[*class_index as usize]),
                // スタック: [total]
                Instruction::Discard,
                Instruction::Push(WsNumber(*class_index)),
                Instruction::Push(WsNumber(*block_size)),
                Instruction::Jump(fsba_labels::FSBA_ALLOC_COMMON),
            ]);
        }

        // === FSBA alloc 共通パス ===
        self.generate_fsba_alloc_common(&mut prog);

        // === 汎用アロケータ (First-Fit + バンプ) ===
        self.generate_general_alloc(&mut prog);

        prog
    }

    /// FSBA alloc 共通パスを生成
    ///
    /// スタック入力: [class_index, class_size]
    /// スタック出力: [ptr]
    fn generate_fsba_alloc_common(&self, prog: &mut WsProgram) {
        prog.push(Instruction::Label(fsba_labels::FSBA_ALLOC_COMMON));
        // スタック: [class_index, class_size]

        // free_head_addr = heap[FSBA_TABLE_PTR] + class_index
        prog.extend([
            Instruction::Swap,
            // スタック: [class_size, class_index]
            Instruction::Push(WsNumber(heap_layout::FSBA_TABLE_PTR)),
            Instruction::Retrieve,
            // スタック: [class_size, class_index, table_ptr]
            Instruction::Add,
            // スタック: [class_size, fha]
        ]);

        // free_head = heap[fha]
        prog.extend([
            Instruction::Duplicate,
            // スタック: [class_size, fha, fha]
            Instruction::Retrieve,
            // スタック: [class_size, fha, free_head]
            Instruction::Duplicate,
            // スタック: [class_size, fha, fh, fh]
            Instruction::JumpIfZero(fsba_labels::FSBA_ALLOC_BUMP),
            // NOT taken: fh != 0. jz popped fh copy.
            // スタック: [class_size, fha, fh]
        ]);

        // === Pop from free list ===
        // Goal: heap[fha] = heap[fh + 1]; return fh + 1
        prog.extend([
            // Read next: heap[fh + 1]
            Instruction::Duplicate,
            // スタック: [cs, fha, fh, fh]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [cs, fha, fh, fh+1]
            Instruction::Retrieve,
            // スタック: [cs, fha, fh, next]
            // Store: heap[fha] = next
            Instruction::Copy(WsNumber(2)),
            // スタック: [cs, fha, fh, next, fha] (copy depth 2: 0=next, 1=fh, 2=fha)
            Instruction::Swap,
            // スタック: [cs, fha, fh, fha, next]
            Instruction::Store,
            // heap[fha] = next. スタック: [cs, fha, fh]
            // Clean up: discard fha and cs, keep fh
            Instruction::Swap,
            // スタック: [cs, fh, fha]
            Instruction::Discard,
            // スタック: [cs, fh]
            Instruction::Swap,
            // スタック: [fh, cs]
            Instruction::Discard,
            // スタック: [fh]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [fh + 1] = [ptr]
            Instruction::Return,
        ]);

        // === FSBA ALLOC BUMP: free list 空 → バンプ拡張 ===
        prog.push(Instruction::Label(fsba_labels::FSBA_ALLOC_BUMP));
        // jz popped fh copy. スタック: [class_size, fha, fh(=0)]
        prog.extend([
            Instruction::Discard,
            // スタック: [class_size, fha]
            Instruction::Discard,
            // スタック: [class_size]
            // block = heap[ALLOC_HEAP_TOP]
            Instruction::Push(WsNumber(heap_layout::ALLOC_HEAP_TOP)),
            Instruction::Retrieve,
            // スタック: [class_size, block]
            // heap[block] = class_size (ヘッダー)
            Instruction::Duplicate,
            // スタック: [cs, block, block]
            Instruction::Copy(WsNumber(2)),
            // スタック: [cs, block, block, cs] (copy depth 2: 0=block, 1=block, 2=cs)
            Instruction::Store,
            // heap[block] = cs. スタック: [cs, block]
            // heap[ALLOC_HEAP_TOP] = block + class_size
            Instruction::Swap,
            // スタック: [block, cs]
            Instruction::Copy(WsNumber(1)),
            // スタック: [block, cs, block] (copy depth 1: 0=cs, 1=block)
            Instruction::Add,
            // スタック: [block, cs + block]
            Instruction::Push(WsNumber(heap_layout::ALLOC_HEAP_TOP)),
            Instruction::Swap,
            // スタック: [block, &AHT, new_top]
            Instruction::Store,
            // heap[AHT] = new_top. スタック: [block]
            // return block + 1
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [block + 1] = [ptr]
            Instruction::Return,
        ]);
    }

    /// 汎用アロケータ (First-Fit + バンプ) を生成
    ///
    /// スタック入力: [total] (total > 32)
    /// スタック出力: [ptr]
    fn generate_general_alloc(&self, prog: &mut WsProgram) {
        prog.push(Instruction::Label(fsba_labels::GENERAL_ALLOC));
        // スタック: [total]

        // total を TEMP_PTR に退避（ループ中に使うため）
        prog.extend([
            Instruction::Duplicate,
            // スタック: [total, total]
            Instruction::Push(WsNumber(heap_layout::TEMP_PTR)),
            Instruction::Swap,
            // スタック: [total, &TEMP, total]
            Instruction::Store,
            // heap[TEMP] = total. スタック: [total]
            Instruction::Discard,
            // スタック: []
        ]);

        // prev_next_addr = ALLOC_FREE_HEAD (5)
        prog.push(Instruction::Push(WsNumber(heap_layout::ALLOC_FREE_HEAD)));
        // スタック: [pna]

        // === First-Fit ループ ===
        prog.push(Instruction::Label(fsba_labels::GENERAL_ALLOC_LOOP));
        // スタック: [pna]
        prog.extend([
            Instruction::Duplicate,
            // スタック: [pna, pna]
            Instruction::Retrieve,
            // スタック: [pna, curr]
            Instruction::Duplicate,
            // スタック: [pna, curr, curr]
            Instruction::JumpIfZero(fsba_labels::GENERAL_ALLOC_BUMP),
            // NOT taken: curr != 0. スタック: [pna, curr]
        ]);

        // curr_size >= total ?
        prog.extend([
            Instruction::Duplicate,
            // スタック: [pna, curr, curr]
            Instruction::Retrieve,
            // スタック: [pna, curr, curr_size]
            Instruction::Push(WsNumber(heap_layout::TEMP_PTR)),
            Instruction::Retrieve,
            // スタック: [pna, curr, curr_size, total]
            Instruction::Sub,
            // スタック: [pna, curr, curr_size - total] = [pna, curr, diff]
            Instruction::Duplicate,
            // スタック: [pna, curr, diff, diff]
            Instruction::JumpIfNegative(fsba_labels::GENERAL_ALLOC_NEXT),
            // NOT taken: diff >= 0 → found!
            Instruction::Jump(fsba_labels::GENERAL_ALLOC_FOUND),
        ]);

        // === NEXT: 次のブロックへ ===
        prog.push(Instruction::Label(fsba_labels::GENERAL_ALLOC_NEXT));
        // jn popped diff copy. スタック: [pna, curr, diff]
        prog.extend([
            Instruction::Discard,
            // スタック: [pna, curr]
            // new pna = curr + 1 (next ポインタのアドレス)
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [pna, curr+1]
            Instruction::Swap,
            // スタック: [curr+1, pna]
            Instruction::Discard,
            // スタック: [curr+1] = [new_pna]
            Instruction::Jump(fsba_labels::GENERAL_ALLOC_LOOP),
        ]);

        // === FOUND: ブロック発見 ===
        prog.push(Instruction::Label(fsba_labels::GENERAL_ALLOC_FOUND));
        // スタック: [pna, curr, diff]
        // diff = curr_size - total
        // Check if diff >= SPLIT_MIN_REMAINDER for splitting
        prog.extend([
            Instruction::Duplicate,
            // スタック: [pna, curr, diff, diff]
            Instruction::Push(WsNumber(alloc_spec::SPLIT_MIN_REMAINDER)),
            Instruction::Sub,
            // スタック: [pna, curr, diff, diff-SPLIT_MIN_REMAINDER]
            Instruction::JumpIfNegative(fsba_labels::GENERAL_ALLOC_NO_SPLIT),
            // NOT taken: diff >= 2 → split
        ]);

        // === SPLIT ===
        // スタック: [pna, curr, diff]
        // 0=diff, 1=curr, 2=pna
        // remainder = curr + total
        prog.extend([
            Instruction::Copy(WsNumber(1)),
            // スタック: [pna, curr, diff, curr]
            Instruction::Push(WsNumber(heap_layout::TEMP_PTR)),
            Instruction::Retrieve,
            // スタック: [pna, curr, diff, curr, total]
            Instruction::Add,
            // スタック: [pna, curr, diff, remainder]
        ]);

        // heap[remainder] = diff (remainder block size)
        prog.extend([
            Instruction::Duplicate,
            // スタック: [pna, curr, diff, rem, rem]
            Instruction::Copy(WsNumber(2)),
            // スタック: [pna, curr, diff, rem, rem, diff] (copy depth 2: 0=rem, 1=rem, 2=diff)
            Instruction::Store,
            // heap[rem] = diff. スタック: [pna, curr, diff, rem]
        ]);

        // heap[remainder + 1] = heap[curr + 1] (copy next pointer)
        prog.extend([
            Instruction::Duplicate,
            // スタック: [pna, curr, diff, rem, rem]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [pna, curr, diff, rem, rem+1]
            Instruction::Copy(WsNumber(3)),
            // スタック: [pna, curr, diff, rem, rem+1, curr] (copy depth 3: 0=rem+1, 1=rem, 2=diff, 3=curr)
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [pna, curr, diff, rem, rem+1, curr+1]
            Instruction::Retrieve,
            // スタック: [pna, curr, diff, rem, rem+1, next]
            Instruction::Store,
            // heap[rem+1] = next. スタック: [pna, curr, diff, rem]
        ]);

        // heap[curr] = total (shrink current block)
        prog.extend([
            Instruction::Copy(WsNumber(2)),
            // スタック: [pna, curr, diff, rem, curr] (copy depth 2: 0=rem, 1=diff, 2=curr)
            Instruction::Push(WsNumber(heap_layout::TEMP_PTR)),
            Instruction::Retrieve,
            // スタック: [pna, curr, diff, rem, curr, total]
            Instruction::Store,
            // heap[curr] = total. スタック: [pna, curr, diff, rem]
        ]);

        // heap[pna] = remainder (link prev to remainder)
        prog.extend([
            Instruction::Copy(WsNumber(3)),
            // スタック: [pna, curr, diff, rem, pna] (copy depth 3: 0=rem, 1=diff, 2=curr, 3=pna)
            Instruction::Swap,
            // スタック: [pna, curr, diff, pna, rem]
            Instruction::Store,
            // heap[pna] = rem. スタック: [pna, curr, diff]
        ]);

        // Clean up and return curr + 1
        prog.extend([
            Instruction::Discard,
            // スタック: [pna, curr]
            Instruction::Swap,
            // スタック: [curr, pna]
            Instruction::Discard,
            // スタック: [curr]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [curr + 1] = [ptr]
            Instruction::Return,
        ]);

        // === NO SPLIT: ブロック全体を使用 ===
        prog.push(Instruction::Label(fsba_labels::GENERAL_ALLOC_NO_SPLIT));
        // jn popped diff-2. スタック: [pna, curr, diff]
        prog.extend([
            Instruction::Discard,
            // スタック: [pna, curr]
            // heap[pna] = heap[curr + 1] (remove curr from free list)
            Instruction::Duplicate,
            // スタック: [pna, curr, curr]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [pna, curr, curr+1]
            Instruction::Retrieve,
            // スタック: [pna, curr, next]
            Instruction::Copy(WsNumber(2)),
            // スタック: [pna, curr, next, pna] (copy depth 2: 0=next, 1=curr, 2=pna)
            Instruction::Swap,
            // スタック: [pna, curr, pna, next]
            Instruction::Store,
            // heap[pna] = next. スタック: [pna, curr]
            // Return curr + 1
            Instruction::Swap,
            // スタック: [curr, pna]
            Instruction::Discard,
            // スタック: [curr]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [curr + 1] = [ptr]
            Instruction::Return,
        ]);

        // === BUMP: フリーリストに適合ブロックなし ===
        prog.push(Instruction::Label(fsba_labels::GENERAL_ALLOC_BUMP));
        // jz popped curr copy. スタック: [pna, curr(=0)]
        prog.extend([
            Instruction::Discard,
            // スタック: [pna]
            Instruction::Discard,
            // スタック: []
            // total = heap[TEMP_PTR]
            Instruction::Push(WsNumber(heap_layout::TEMP_PTR)),
            Instruction::Retrieve,
            // スタック: [total]
            // block = heap[ALLOC_HEAP_TOP]
            Instruction::Push(WsNumber(heap_layout::ALLOC_HEAP_TOP)),
            Instruction::Retrieve,
            // スタック: [total, block]
            // heap[block] = total (ヘッダー)
            Instruction::Duplicate,
            // スタック: [total, block, block]
            Instruction::Copy(WsNumber(2)),
            // スタック: [total, block, block, total]
            Instruction::Store,
            // heap[block] = total. スタック: [total, block]
            // heap[ALLOC_HEAP_TOP] = block + total
            Instruction::Swap,
            // スタック: [block, total]
            Instruction::Copy(WsNumber(1)),
            // スタック: [block, total, block]
            Instruction::Add,
            // スタック: [block, total + block]
            Instruction::Push(WsNumber(heap_layout::ALLOC_HEAP_TOP)),
            Instruction::Swap,
            // スタック: [block, &AHT, new_top]
            Instruction::Store,
            // heap[AHT] = new_top. スタック: [block]
            // return block + 1
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [block + 1] = [ptr]
            Instruction::Return,
        ]);
    }

    /// __rt_free サブルーチンを生成
    fn generate_rt_free(&self) -> WsProgram {
        let mut prog = WsProgram::new();

        // === __rt_free(ptr) ===
        // スタック入力: [ptr]
        // スタック出力: []
        prog.push(Instruction::Label(reserved_labels::RT_FREE));
        // block = ptr - 1
        prog.extend([
            Instruction::Push(WsNumber(1)),
            Instruction::Sub,
            // スタック: [block]
            // block_size = heap[block]
            Instruction::Duplicate,
            // スタック: [block, block]
            Instruction::Retrieve,
            // スタック: [block, block_size]
        ]);

        // サイズクラスカスケード (exact match)
        // 各クラスについて: dup; push class_size; sub; jz FREE_CLASS_N
        let class_sizes = alloc_spec::FSBA_BLOCK_SIZES;
        for (i, &size) in class_sizes.iter().enumerate() {
            prog.extend([
                Instruction::Duplicate,
                // スタック: [block, bs, bs]
                Instruction::Push(WsNumber(size)),
                Instruction::Sub,
                // スタック: [block, bs, bs - size]
                Instruction::JumpIfZero(FREE_CLASS_LABELS[i]),
                // NOT taken: bs != size. jz popped result.
                // スタック: [block, bs]
            ]);
        }

        // No match → general free
        prog.extend([
            Instruction::Discard,
            // スタック: [block]
            Instruction::Jump(fsba_labels::GENERAL_FREE),
        ]);

        // === Free クラスハンドラ ===
        for (i, &_size) in class_sizes.iter().enumerate() {
            prog.extend([
                Instruction::Label(FREE_CLASS_LABELS[i]),
                // jz popped (bs - size). スタック: [block, bs]
                Instruction::Discard,
                // スタック: [block]
                Instruction::Push(WsNumber(i as i64)),
                // スタック: [block, class_index]
                Instruction::Jump(fsba_labels::FSBA_FREE_COMMON),
            ]);
        }

        // === FSBA free 共通パス ===
        prog.push(Instruction::Label(fsba_labels::FSBA_FREE_COMMON));
        // スタック: [block, class_index]
        prog.extend([
            // free_head_addr = heap[FSBA_TABLE_PTR] + class_index
            Instruction::Push(WsNumber(heap_layout::FSBA_TABLE_PTR)),
            Instruction::Retrieve,
            // スタック: [block, class_index, table_ptr]
            Instruction::Add,
            // スタック: [block, fha]
            // heap[block + 1] = heap[fha] (current head → next)
            Instruction::Copy(WsNumber(1)),
            // スタック: [block, fha, block]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [block, fha, block+1]
            Instruction::Copy(WsNumber(1)),
            // スタック: [block, fha, block+1, fha]
            Instruction::Retrieve,
            // スタック: [block, fha, block+1, old_head]
            Instruction::Store,
            // heap[block+1] = old_head. スタック: [block, fha]
            // heap[fha] = block
            Instruction::Swap,
            // スタック: [fha, block]
            Instruction::Store,
            // heap[fha] = block. スタック: []
            Instruction::Return,
        ]);

        // === General free ===
        prog.push(Instruction::Label(fsba_labels::GENERAL_FREE));
        // スタック: [block]
        prog.extend([
            // heap[block + 1] = heap[ALLOC_FREE_HEAD]
            Instruction::Duplicate,
            // スタック: [block, block]
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [block, block+1]
            Instruction::Push(WsNumber(heap_layout::ALLOC_FREE_HEAD)),
            Instruction::Retrieve,
            // スタック: [block, block+1, old_head]
            Instruction::Store,
            // heap[block+1] = old_head. スタック: [block]
            // heap[ALLOC_FREE_HEAD] = block
            Instruction::Push(WsNumber(heap_layout::ALLOC_FREE_HEAD)),
            Instruction::Swap,
            // スタック: [&AFH, block]
            Instruction::Store,
            // heap[AFH] = block. スタック: []
            Instruction::Return,
        ]);

        prog
    }
}

impl AllocRuntime for FsbaFirstFitAllocRuntime {
    fn generate_memory_init(&self, global_heap_size: i64) -> WsProgram {
        let managed_start = heap_layout::GLOBAL_PTR + global_heap_size;
        let mut prog = WsProgram::new();

        // heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR
        // グローバルスコープのローカル変数アクセスとの互換性のため
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Push(WsNumber(heap_layout::GLOBAL_PTR)),
            Instruction::Store,
        ]);

        // heap[ALLOC_FREE_HEAD] = 0 (汎用フリーリスト空)
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::ALLOC_FREE_HEAD)),
            Instruction::Push(WsNumber(0)),
            Instruction::Store,
        ]);

        // heap[FSBA_TABLE_PTR] = managed_start
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::FSBA_TABLE_PTR)),
            Instruction::Push(WsNumber(managed_start)),
            Instruction::Store,
        ]);

        // FSBA テーブル初期化: heap[managed_start + i] = 0 (各フリーリスト空)
        for i in 0..heap_layout::FSBA_CLASS_COUNT {
            prog.extend([
                Instruction::Push(WsNumber(managed_start + i)),
                Instruction::Push(WsNumber(0)),
                Instruction::Store,
            ]);
        }

        // heap[ALLOC_HEAP_TOP] = managed_start + FSBA_CLASS_COUNT
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::ALLOC_HEAP_TOP)),
            Instruction::Push(WsNumber(managed_start + heap_layout::FSBA_CLASS_COUNT)),
            Instruction::Store,
        ]);

        prog
    }

    fn generate_subroutines(&self) -> WsProgram {
        let mut prog = WsProgram::new();
        prog.append(self.generate_rt_alloc());
        prog.append(self.generate_rt_free());
        prog
    }

    fn generate_function_prologue(&self, local_heap_size: i64, arg_offsets: &[i64]) -> WsProgram {
        // BumpAllocRuntime と同じフロー（generate_common_prologue を使用）
        generate_common_prologue(local_heap_size, arg_offsets)
    }

    fn generate_function_epilogue(&self) -> WsProgram {
        // BumpAllocRuntime と同じフロー（generate_common_epilogue を使用）
        generate_common_epilogue()
    }
}

#[cfg(test)]
#[path = "fsba_tests.rs"]
mod tests;
