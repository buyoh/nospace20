//! ランタイムメモリアロケータのコード生成
//!
//! `AllocRuntime` trait は、Whitespace コンパイル時のメモリ管理コード生成を抽象化する。
//! 実装を差し替えることで、異なるメモリ管理方式（バンプ方式、FSBA 等）を選択可能にする。
//!
//! 全ての `AllocRuntime` 実装は `__rt_alloc(size) → ptr` / `__rt_free(ptr)` サブルーチンを
//! 提供しなければならない。スタックフレーム確保もこれらのサブルーチンを経由する。

use crate::compiler_ws::{
    instruction::Instruction, label::reserved_labels, memory::heap_layout, program::WsProgram,
    types::WsNumber,
};

/// ランタイムメモリアロケータの WS コード生成を担当する trait。
///
/// 各メソッドは Whitespace 命令列 (`WsProgram`) を返す。
/// 実装を差し替えることで、異なるメモリ管理方式を選択可能にする。
pub trait AllocRuntime {
    /// ヘッダー部分のメモリ初期化コードを生成。
    ///
    /// ヒープの予約アドレスを初期化する。
    /// `global_heap_size`: グローバル変数 + static 変数の合計サイズ
    fn generate_memory_init(&self, global_heap_size: i64) -> WsProgram;

    /// フッター部分のサブルーチン定義コードを生成。
    ///
    /// アロケータが使用するサブルーチン（`__rt_alloc`, `__rt_free` 等）を定義する。
    /// 全ての実装は最低限 `__rt_alloc` と `__rt_free` を定義しなければならない。
    fn generate_subroutines(&self) -> WsProgram;

    /// 関数プロローグ: 引数コピー + フレーム確保
    ///
    /// スタック入力: `[..., arg(n-1), ..., arg(0)]`
    /// スタック出力: `[..., old_context]`
    ///
    /// 呼び出し後:
    /// - `heap[LOCAL_HEAP_BEGIN]` = 新フレーム先頭アドレス
    /// - 引数は `heap[LOCAL_HEAP_BEGIN + arg_offsets[i]]` に格納済み
    /// - `old_context` はエピローグで使用するコンテキスト復元データ
    fn generate_function_prologue(&self, local_heap_size: i64, arg_offsets: &[i64]) -> WsProgram;

    /// 関数エピローグ: フレーム解放 + コンテキスト復元
    ///
    /// スタック入力: `[..., old_context]`
    /// スタック出力: `[...]`
    fn generate_function_epilogue(&self) -> WsProgram;
}

/// バンプアロケータ（現行方式）
///
/// `LOCAL_HEAP_BEGIN` / `LOCAL_HEAP_END` によるバンプ方式のメモリ管理。
/// `--std-ext alloc` 未指定時のデフォルト動作。
pub struct BumpAllocRuntime;

impl AllocRuntime for BumpAllocRuntime {
    fn generate_memory_init(&self, global_heap_size: i64) -> WsProgram {
        let mut prog = WsProgram::new();

        // heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Push(WsNumber(heap_layout::GLOBAL_PTR)),
            Instruction::Store,
        ]);

        // heap[LOCAL_HEAP_END] = GLOBAL_PTR + global_heap_size
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Push(WsNumber(heap_layout::GLOBAL_PTR + global_heap_size)),
            Instruction::Store,
        ]);

        prog
    }

    fn generate_subroutines(&self) -> WsProgram {
        let mut prog = WsProgram::new();

        // __rt_alloc(size) → ptr
        // スタック入力: [size]
        // スタック出力: [ptr]
        // ptr = heap[LOCAL_HEAP_END]; heap[LOCAL_HEAP_END] = ptr + size; return ptr
        prog.extend([
            Instruction::Label(reserved_labels::RT_ALLOC),
            // スタック: [size]
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Retrieve,
            // スタック: [size, LHE_val]
            Instruction::Swap,
            // スタック: [LHE_val, size]
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            // スタック: [LHE_val, size, &LHE]
            Instruction::Copy(WsNumber(2)),
            // スタック: [LHE_val, size, &LHE, LHE_val]
            Instruction::Copy(WsNumber(2)),
            // スタック: [LHE_val, size, &LHE, LHE_val, size]
            Instruction::Add,
            // スタック: [LHE_val, size, &LHE, LHE_val+size]
            Instruction::Store,
            // スタック: [LHE_val, size]  heap[LHE] = LHE_val + size
            Instruction::Discard,
            // スタック: [LHE_val]  ← ptr
            Instruction::Return,
        ]);

        // __rt_free(ptr)
        // スタック入力: [ptr]
        // スタック出力: []
        // heap[LOCAL_HEAP_END] = ptr (LIFO バンプ方式)
        prog.extend([
            Instruction::Label(reserved_labels::RT_FREE),
            // スタック: [ptr]
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            // スタック: [ptr, &LHE]
            Instruction::Swap,
            // スタック: [&LHE, ptr]
            Instruction::Store,
            // heap[LHE] = ptr
            Instruction::Return,
        ]);

        prog
    }

    fn generate_function_prologue(&self, local_heap_size: i64, arg_offsets: &[i64]) -> WsProgram {
        let mut prog = WsProgram::new();

        // 1. 現在の LOCAL_HEAP_BEGIN をスタックに退避（old_context）
        //    引数の下に配置するため、先に退避してから引数を処理する
        //    スタック: [arg(n-1), ..., arg(0)] → [arg(n-1), ..., arg(0), old_LHB]
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
        ]);

        // 2. __rt_alloc(local_heap_size) を呼び出し: ptr を取得
        //    スタック: [..., old_LHB] → [..., old_LHB, ptr]
        prog.extend([
            Instruction::Push(WsNumber(local_heap_size)),
            Instruction::Call(reserved_labels::RT_ALLOC),
        ]);

        // 3. LOCAL_HEAP_BEGIN = ptr
        //    スタック: [..., old_LHB, ptr] → [..., old_LHB]
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Swap,
            // スタック: [..., old_LHB, &LHB, ptr]
            Instruction::Store,
            // heap[LHB] = ptr
        ]);

        // 4. 引数を heap[LOCAL_HEAP_BEGIN + offset] にコピー
        //    スタック: [arg(n-1), ..., arg(0), old_LHB]
        //    old_LHB は引数の下に位置しているが、スタック上では引数が先に
        //    push され、old_LHB が後から push されている。
        //    引数コピーには swap で old_LHB を一時退避して引数にアクセスする。
        //    引数は offset の逆順（スタックトップから: arg(0) の offset が最後）で
        //    処理する。
        //
        //    ただし old_LHB はスタックトップにあるため、各引数の処理時に swap が必要。
        for offset in arg_offsets.iter().rev() {
            // スタック: [..., arg_i, old_LHB]
            prog.extend([
                Instruction::Swap,
                // スタック: [..., old_LHB, arg_i]
                Instruction::Push(WsNumber(*offset)),
                Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
                Instruction::Retrieve,
                Instruction::Add,
                // スタック: [..., old_LHB, arg_i, LHB+offset]
                Instruction::Swap,
                // スタック: [..., old_LHB, LHB+offset, arg_i]
                Instruction::Store,
                // heap[LHB+offset] = arg_i
                // スタック: [..., old_LHB]
            ]);
        }

        // スタック出力: [..., old_LHB]
        prog
    }

    fn generate_function_epilogue(&self) -> WsProgram {
        let mut prog = WsProgram::new();

        // スタック入力: [old_LHB]

        // 1. ptr = LOCAL_HEAP_BEGIN
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            // スタック: [old_LHB, ptr]
        ]);

        // 2. LOCAL_HEAP_BEGIN = old_LHB
        prog.extend([
            Instruction::Swap,
            // スタック: [ptr, old_LHB]
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Swap,
            // スタック: [ptr, &LHB, old_LHB]
            Instruction::Store,
            // heap[LHB] = old_LHB
            // スタック: [ptr]
        ]);

        // 3. __rt_free(ptr)
        prog.extend([
            Instruction::Call(reserved_labels::RT_FREE),
            // スタック: []
        ]);

        prog
    }
}

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
const FSBA_SIZE_CLASSES: [(i64, i64, i64); 5] = [
    (0, 2, 3),   // class 0: block_size=2,  threshold=3  → total <= 2
    (1, 4, 5),   // class 1: block_size=4,  threshold=5  → total <= 4
    (2, 8, 9),   // class 2: block_size=8,  threshold=9  → total <= 8
    (3, 16, 17), // class 3: block_size=16, threshold=17 → total <= 16
    (4, 32, 33), // class 4: block_size=32, threshold=33 → total <= 32
];

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
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            // スタック: [size+1]
            Instruction::Duplicate,
            // スタック: [size+1, size+1]
            Instruction::Push(WsNumber(2)),
            Instruction::Sub,
            // スタック: [size+1, size+1-2]
            Instruction::JumpIfNegative(fsba_labels::ALLOC_SET_MIN),
            // size+1 >= 2: total = size+1
            Instruction::Jump(fsba_labels::ALLOC_CASCADE),
        ]);

        // ALLOC_SET_MIN: total < 2 → total = 2
        prog.extend([
            Instruction::Label(fsba_labels::ALLOC_SET_MIN),
            // jn popped size+1-2. Stack: [size+1]
            Instruction::Discard,
            Instruction::Push(WsNumber(2)),
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
        // Check if diff >= 2 for splitting
        prog.extend([
            Instruction::Duplicate,
            // スタック: [pna, curr, diff, diff]
            Instruction::Push(WsNumber(2)),
            Instruction::Sub,
            // スタック: [pna, curr, diff, diff-2]
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
        let class_sizes: [i64; 5] = [2, 4, 8, 16, 32];
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
        // BumpAllocRuntime と同じフロー:
        // 1. old_LHB をスタックに退避
        // 2. __rt_alloc(local_heap_size) → ptr
        // 3. LOCAL_HEAP_BEGIN = ptr
        // 4. 引数コピー
        let mut prog = WsProgram::new();

        // 1. old_LHB を退避
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
        ]);

        // 2. __rt_alloc(local_heap_size)
        prog.extend([
            Instruction::Push(WsNumber(local_heap_size)),
            Instruction::Call(reserved_labels::RT_ALLOC),
        ]);

        // 3. LOCAL_HEAP_BEGIN = ptr
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Swap,
            Instruction::Store,
        ]);

        // 4. 引数コピー
        for offset in arg_offsets.iter().rev() {
            prog.extend([
                Instruction::Swap,
                Instruction::Push(WsNumber(*offset)),
                Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
                Instruction::Retrieve,
                Instruction::Add,
                Instruction::Swap,
                Instruction::Store,
            ]);
        }

        prog
    }

    fn generate_function_epilogue(&self) -> WsProgram {
        // BumpAllocRuntime と同じフロー:
        // 1. ptr = LOCAL_HEAP_BEGIN
        // 2. LOCAL_HEAP_BEGIN = old_LHB
        // 3. __rt_free(ptr)
        let mut prog = WsProgram::new();

        // 1. ptr = LOCAL_HEAP_BEGIN
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
        ]);

        // 2. LOCAL_HEAP_BEGIN = old_LHB
        prog.extend([
            Instruction::Swap,
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Swap,
            Instruction::Store,
        ]);

        // 3. __rt_free(ptr)
        prog.push(Instruction::Call(reserved_labels::RT_FREE));

        prog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_memory_init_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_memory_init(5);
        assert!(!prog.is_empty(), "memory init should produce instructions");
    }

    #[test]
    fn test_bump_subroutines_has_alloc_and_free() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_subroutines();
        assert!(
            !prog.is_empty(),
            "bump allocator should have __rt_alloc/__rt_free subroutines"
        );
        // サブルーチンには Label, Return が含まれるはず
        let insts = prog.instructions();
        let has_rt_alloc_label = insts
            .iter()
            .any(|i| matches!(i, Instruction::Label(label) if *label == reserved_labels::RT_ALLOC));
        let has_rt_free_label = insts
            .iter()
            .any(|i| matches!(i, Instruction::Label(label) if *label == reserved_labels::RT_FREE));
        assert!(has_rt_alloc_label, "should contain __rt_alloc label");
        assert!(has_rt_free_label, "should contain __rt_free label");
    }

    #[test]
    fn test_bump_prologue_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_prologue(3, &[0, 1, 2]);
        assert!(!prog.is_empty(), "prologue should produce instructions");
    }

    #[test]
    fn test_bump_prologue_calls_rt_alloc() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_prologue(5, &[0, 1]);
        let insts = prog.instructions();
        let has_alloc_call = insts
            .iter()
            .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_ALLOC));
        assert!(has_alloc_call, "prologue should call __rt_alloc");
    }

    #[test]
    fn test_bump_epilogue_produces_instructions() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_epilogue();
        assert!(!prog.is_empty(), "epilogue should produce instructions");
    }

    #[test]
    fn test_bump_epilogue_calls_rt_free() {
        let bump = BumpAllocRuntime;
        let prog = bump.generate_function_epilogue();
        let insts = prog.instructions();
        let has_free_call = insts
            .iter()
            .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_FREE));
        assert!(has_free_call, "epilogue should call __rt_free");
    }

    #[test]
    fn test_bump_prologue_no_args() {
        let bump = BumpAllocRuntime;
        // 引数なしの場合もプロローグは正しく生成されること
        let prog = bump.generate_function_prologue(5, &[]);
        assert!(
            !prog.is_empty(),
            "prologue with no args should produce instructions"
        );
    }

    /// VM 上で __rt_alloc → __rt_free → __rt_alloc を実行し、バンプ方式の動作を検証する。
    /// alloc → free → alloc の LIFO パターンで同じアドレスが返ることを確認。
    #[test]
    fn test_bump_alloc_free_on_vm() {
        use crate::whitespace::{StepResult, WhitespaceVM};

        let bump = BumpAllocRuntime;
        let mut prog = WsProgram::new();

        // メモリ初期化 (global_heap_size = 0)
        prog.append(bump.generate_memory_init(0));

        // __rt_alloc(3): ptr1 を確保
        prog.extend([
            Instruction::Push(WsNumber(3)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            // ptr1 がスタックトップ: 期待値は GLOBAL_PTR (= 8)
            Instruction::OutputNumber, // print ptr1
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar, // '\n'
        ]);

        // __rt_alloc(2): ptr2 を確保
        prog.extend([
            Instruction::Push(WsNumber(2)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            // ptr2 がスタックトップ: 期待値は 8 + 3 = 11
            Instruction::OutputNumber, // print ptr2
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar, // '\n'
        ]);

        // __rt_free(ptr2): ptr2 を解放 (LHE = 11)
        prog.extend([
            Instruction::Push(WsNumber(11)),
            Instruction::Call(reserved_labels::RT_FREE),
        ]);

        // __rt_alloc(2): ptr3 を確保 (同じアドレス 11 が返るはず)
        prog.extend([
            Instruction::Push(WsNumber(2)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            Instruction::OutputNumber, // print ptr3
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar, // '\n'
        ]);

        prog.push(Instruction::Exit);
        prog.append(bump.generate_subroutines());

        let stdout = Vec::<u8>::new();
        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap()
            .with_io(Box::new(std::io::Cursor::new(Vec::new())), Box::new(stdout));
        let result = vm.run(10000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        let output = vm.get_stdout_string();
        assert_eq!(output, "8\n11\n11\n");
    }

    /// プロローグ → エピローグの完全フローをVM上で検証する。
    /// 関数呼び出し相当のフレーム確保→引数書き込み→読み出し→解放をシミュレート。
    #[test]
    fn test_bump_prologue_epilogue_on_vm() {
        use crate::whitespace::{StepResult, WhitespaceVM};

        let bump = BumpAllocRuntime;
        let mut prog = WsProgram::new();

        // メモリ初期化 (global_heap_size = 2)
        // LHB = GLOBAL_PTR(8), LHE = GLOBAL_PTR + 2 = 10
        prog.append(bump.generate_memory_init(2));

        // 呼び出し規約: 引数は順序通りに push（arg(0)が最も深い位置）
        prog.extend([
            Instruction::Push(WsNumber(42)), // arg(0) - 先に push（深い）
            Instruction::Push(WsNumber(99)), // arg(1) - 後に push（トップ）
        ]);

        // プロローグ: local_heap_size=4, arg_offsets=[0, 1]
        prog.append(bump.generate_function_prologue(4, &[0, 1]));
        // スタック: [old_LHB(=8)]
        // LHB = 10 (新フレーム先頭)
        // heap[10] = 42 (arg(0)), heap[11] = 99 (arg(1))

        // 引数を読み取って出力
        prog.extend([
            // print heap[LHB + 0] = 42
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve, // LHB = 10
            Instruction::Push(WsNumber(0)),
            Instruction::Add,
            Instruction::Retrieve, // heap[10] = 42
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
            // print heap[LHB + 1] = 99
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve, // LHB = 10
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            Instruction::Retrieve, // heap[11] = 99
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        // エピローグ: old_LHB はスタックにある
        prog.append(bump.generate_function_epilogue());

        // LHB が元の値 (8) に復元されたことを確認
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            Instruction::OutputNumber, // print LHB (should be 8)
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        // LHE がフレーム先頭 (10) に戻ったことを確認
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_END)),
            Instruction::Retrieve,
            Instruction::OutputNumber, // print LHE (should be 10)
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        prog.push(Instruction::Exit);
        prog.append(bump.generate_subroutines());

        let stdout = Vec::<u8>::new();
        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap()
            .with_io(Box::new(std::io::Cursor::new(Vec::new())), Box::new(stdout));
        let result = vm.run(10000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        let output = vm.get_stdout_string();
        assert_eq!(output, "42\n99\n8\n10\n");
    }

    // === FsbaFirstFitAllocRuntime テスト ===

    #[test]
    fn test_fsba_memory_init_produces_instructions() {
        let fsba = FsbaFirstFitAllocRuntime;
        let prog = fsba.generate_memory_init(5);
        assert!(!prog.is_empty(), "memory init should produce instructions");
    }

    #[test]
    fn test_fsba_subroutines_has_alloc_and_free() {
        let fsba = FsbaFirstFitAllocRuntime;
        let prog = fsba.generate_subroutines();
        assert!(!prog.is_empty(), "FSBA should have subroutines");
        let insts = prog.instructions();
        let has_rt_alloc_label = insts
            .iter()
            .any(|i| matches!(i, Instruction::Label(label) if *label == reserved_labels::RT_ALLOC));
        let has_rt_free_label = insts
            .iter()
            .any(|i| matches!(i, Instruction::Label(label) if *label == reserved_labels::RT_FREE));
        assert!(has_rt_alloc_label, "should contain __rt_alloc label");
        assert!(has_rt_free_label, "should contain __rt_free label");
    }

    #[test]
    fn test_fsba_prologue_calls_rt_alloc() {
        let fsba = FsbaFirstFitAllocRuntime;
        let prog = fsba.generate_function_prologue(5, &[0, 1]);
        let insts = prog.instructions();
        let has_alloc_call = insts
            .iter()
            .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_ALLOC));
        assert!(has_alloc_call, "prologue should call __rt_alloc");
    }

    #[test]
    fn test_fsba_epilogue_calls_rt_free() {
        let fsba = FsbaFirstFitAllocRuntime;
        let prog = fsba.generate_function_epilogue();
        let insts = prog.instructions();
        let has_free_call = insts
            .iter()
            .any(|i| matches!(i, Instruction::Call(label) if *label == reserved_labels::RT_FREE));
        assert!(has_free_call, "epilogue should call __rt_free");
    }

    /// VM 上で FSBA __rt_alloc → __rt_free → __rt_alloc を実行し、
    /// 同一サイズクラスでのブロック再利用を検証する。
    #[test]
    fn test_fsba_alloc_free_reuse_on_vm() {
        use crate::whitespace::{StepResult, WhitespaceVM};

        let fsba = FsbaFirstFitAllocRuntime;
        let mut prog = WsProgram::new();

        // メモリ初期化 (global_heap_size = 0)
        // managed_start = 8, FSBA table at 8-12, AHT = 13
        prog.append(fsba.generate_memory_init(0));

        // __rt_alloc(1): size=1 → total=2 → class 0 → bump
        // block = 13, ptr = 14
        prog.extend([
            Instruction::Push(WsNumber(1)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        // __rt_free(14): block=13, bs=2 → class 0 free list
        prog.extend([
            Instruction::Push(WsNumber(14)),
            Instruction::Call(reserved_labels::RT_FREE),
        ]);

        // __rt_alloc(1): size=1 → total=2 → class 0 → free list has block 13 → reuse!
        // ptr = 14 (same as before)
        prog.extend([
            Instruction::Push(WsNumber(1)),
            Instruction::Call(reserved_labels::RT_ALLOC),
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        prog.push(Instruction::Exit);
        prog.append(fsba.generate_subroutines());

        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap()
            .with_io(
                Box::new(std::io::Cursor::new(Vec::new())),
                Box::new(Vec::<u8>::new()),
            );
        let result = vm.run(10000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        let output = vm.get_stdout_string();
        // Both allocs return ptr=14 (same block reused)
        assert_eq!(output, "14\n14\n");
    }

    // ===== Reuse efficiency test helpers =====

    /// alloc/free 操作を表す列挙型
    enum AllocOp {
        /// __rt_alloc(size), 結果のポインタをヒープ上の slot に保存
        Alloc { size: i64, slot: i64 },
        /// __rt_free(heap[slot])
        Free { slot: i64 },
    }

    /// slot をヒープアドレスに変換（負のアドレスを使用して衝突を回避）
    fn slot_addr(slot: i64) -> i64 {
        -1000 - slot
    }

    /// alloc/free 操作列を受け取り、実行後の VM を返すヘルパー
    fn run_alloc_free_sequence(
        runtime: &dyn AllocRuntime,
        global_heap_size: i64,
        ops: &[AllocOp],
    ) -> crate::whitespace::WhitespaceVM {
        use crate::whitespace::{StepResult, WhitespaceVM};

        let mut prog = WsProgram::new();
        prog.append(runtime.generate_memory_init(global_heap_size));

        for op in ops {
            match op {
                AllocOp::Alloc { size, slot } => {
                    prog.extend([
                        Instruction::Push(WsNumber(*size)),
                        Instruction::Call(reserved_labels::RT_ALLOC),
                        // stack: [ptr]
                        Instruction::Push(WsNumber(slot_addr(*slot))),
                        Instruction::Swap,
                        Instruction::Store,
                        // heap[slot_addr] = ptr
                    ]);
                }
                AllocOp::Free { slot } => {
                    prog.extend([
                        Instruction::Push(WsNumber(slot_addr(*slot))),
                        Instruction::Retrieve,
                        // stack: [ptr]
                        Instruction::Call(reserved_labels::RT_FREE),
                    ]);
                }
            }
        }

        prog.push(Instruction::Exit);
        prog.append(runtime.generate_subroutines());

        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap()
            .with_io(
                Box::new(std::io::Cursor::new(Vec::new())),
                Box::new(Vec::<u8>::new()),
            );
        let result = vm.run(1_000_000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        vm
    }

    // ===== BumpAllocRuntime reuse efficiency tests =====

    /// LIFO 順で free→alloc したとき、LOCAL_HEAP_END が成長しないことを検証。
    #[test]
    fn test_bump_reuse_lifo_heap_stable() {
        let bump = BumpAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 3, slot: 0 },
            AllocOp::Alloc { size: 2, slot: 1 },
            AllocOp::Free { slot: 1 },
            AllocOp::Alloc { size: 2, slot: 2 },
        ];
        let vm = run_alloc_free_sequence(&bump, 0, &ops);
        let heap = vm.heap();

        // alloc(3): ptr=8, LHE=11
        // alloc(2): ptr=11, LHE=13 (peak)
        // free(11): LHE=11
        // alloc(2): ptr=11, LHE=13 (no growth beyond peak)
        let lhe = heap.get(&heap_layout::LOCAL_HEAP_END).copied().unwrap_or(0);
        assert_eq!(
            lhe,
            heap_layout::GLOBAL_PTR + 3 + 2,
            "LOCAL_HEAP_END should not grow beyond peak after LIFO free+alloc"
        );
    }

    /// ループでの alloc/free（LIFO 順）で LOCAL_HEAP_END が一定に保たれることを検証。
    #[test]
    fn test_bump_reuse_loop_heap_stable() {
        let bump = BumpAllocRuntime;
        let mut ops = vec![AllocOp::Alloc { size: 5, slot: 0 }];
        for _ in 0..10 {
            ops.push(AllocOp::Alloc { size: 3, slot: 1 });
            ops.push(AllocOp::Free { slot: 1 });
        }
        let vm = run_alloc_free_sequence(&bump, 0, &ops);
        let heap = vm.heap();

        // alloc(5): LHE = 8+5 = 13
        // Each loop: alloc(3) → LHE=16, free → LHE=13
        // After all loops LHE should be 13 (same as after alloc(A))
        let lhe = heap.get(&heap_layout::LOCAL_HEAP_END).copied().unwrap_or(0);
        assert_eq!(
            lhe,
            heap_layout::GLOBAL_PTR + 5,
            "LOCAL_HEAP_END should remain stable after LIFO alloc/free loop"
        );
    }

    // ===== FsbaFirstFitAllocRuntime reuse efficiency tests =====

    /// class 0 (サイズ 1) の alloc→free→alloc で ALLOC_HEAP_TOP が成長しないことを検証。
    #[test]
    fn test_fsba_reuse_class0_heap_stable() {
        let fsba = FsbaFirstFitAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 1, slot: 0 },
            AllocOp::Free { slot: 0 },
            AllocOp::Alloc { size: 1, slot: 1 },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        // managed_start=8, table at 8..12, AHT initial=13
        // alloc(1): total=2, class 0, block_size=2, bump: AHT=15
        // free → push to class 0 free list
        // alloc(1): pop from class 0 free list, AHT still 15
        let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
        assert_eq!(
            aht, 15,
            "ALLOC_HEAP_TOP should not grow after class 0 free+alloc"
        );
    }

    /// 各サイズクラス (0-4) について alloc→free→alloc で ALLOC_HEAP_TOP が成長しないことを検証。
    #[test]
    fn test_fsba_reuse_each_class_heap_stable() {
        let fsba = FsbaFirstFitAllocRuntime;
        // (user_size, block_size)
        let classes: [(i64, i64); 5] = [
            (1, 2),   // class 0
            (3, 4),   // class 1
            (7, 8),   // class 2
            (15, 16), // class 3
            (31, 32), // class 4
        ];

        for (user_size, block_size) in &classes {
            let ops = vec![
                AllocOp::Alloc {
                    size: *user_size,
                    slot: 0,
                },
                AllocOp::Free { slot: 0 },
                AllocOp::Alloc {
                    size: *user_size,
                    slot: 1,
                },
            ];
            let vm = run_alloc_free_sequence(&fsba, 0, &ops);
            let heap = vm.heap();

            // AHT initial=13, after first alloc: 13+block_size
            let expected_aht = 13 + block_size;
            let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
            assert_eq!(
                aht, expected_aht,
                "ALLOC_HEAP_TOP should not grow for class with block_size={block_size}"
            );
        }
    }

    /// 異なるリクエストサイズでも同一サイズクラスに切り上げられる場合、
    /// free→alloc で再利用されることを検証。
    #[test]
    fn test_fsba_reuse_roundup_heap_stable() {
        let fsba = FsbaFirstFitAllocRuntime;
        // alloc(2) → total=3, class 1 (block_size=4)
        // free → push to class 1 free list
        // alloc(3) → total=4, class 1, pop from free list
        let ops = vec![
            AllocOp::Alloc { size: 2, slot: 0 },
            AllocOp::Free { slot: 0 },
            AllocOp::Alloc { size: 3, slot: 1 },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        // AHT initial=13, after alloc(2): 13+4=17, after free+alloc(3): 17
        let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
        assert_eq!(
            aht, 17,
            "ALLOC_HEAP_TOP should not grow when round-up reuses same class"
        );
    }

    /// 100 回の alloc/free ループで ALLOC_HEAP_TOP が一定に保たれることを検証。
    #[test]
    fn test_fsba_reuse_loop_heap_stable() {
        let fsba = FsbaFirstFitAllocRuntime;
        let mut ops = Vec::new();
        for _ in 0..100 {
            ops.push(AllocOp::Alloc { size: 3, slot: 0 });
            ops.push(AllocOp::Free { slot: 0 });
        }
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        // alloc(3): total=4, class 1, block_size=4. AHT: 13→17
        // Each loop: alloc pops free list (or bumps first time), free pushes
        // After 100 loops AHT should be 17
        let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
        assert_eq!(
            aht, 17,
            "ALLOC_HEAP_TOP should remain stable after 100 alloc/free loops"
        );
    }

    /// free 後にフリーリストが正しく更新されていることを直接検証。
    #[test]
    fn test_fsba_reuse_freelist_populated() {
        let fsba = FsbaFirstFitAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 1, slot: 0 },
            AllocOp::Free { slot: 0 },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        // FSBA table for class 0 is at heap[table_ptr + 0]
        let table_ptr = heap.get(&heap_layout::FSBA_TABLE_PTR).copied().unwrap_or(0);
        let class0_head = heap.get(&(table_ptr + 0)).copied().unwrap_or(0);
        assert_ne!(
            class0_head, 0,
            "Class 0 free list should be populated after free"
        );
    }

    /// free→alloc でフリーリストからポップされ、リストが空に戻ることを検証。
    #[test]
    fn test_fsba_reuse_freelist_empty_after_realloc() {
        let fsba = FsbaFirstFitAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 1, slot: 0 },
            AllocOp::Free { slot: 0 },
            AllocOp::Alloc { size: 1, slot: 1 },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        let table_ptr = heap.get(&heap_layout::FSBA_TABLE_PTR).copied().unwrap_or(0);
        let class0_head = heap.get(&(table_ptr + 0)).copied().unwrap_or(0);
        assert_eq!(
            class0_head, 0,
            "Class 0 free list should be empty after realloc"
        );
    }

    /// 32 セル超（汎用アロケータ経由）の alloc→free→alloc で ALLOC_HEAP_TOP が成長しないことを検証。
    #[test]
    fn test_fsba_reuse_general_heap_stable() {
        let fsba = FsbaFirstFitAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 40, slot: 0 },
            AllocOp::Free { slot: 0 },
            AllocOp::Alloc { size: 40, slot: 1 },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        // alloc(40): total=41, general bump. AHT: 13→54
        // free → general free list
        // alloc(40): first-fit from free list. AHT stays 54
        let aht = heap.get(&heap_layout::ALLOC_HEAP_TOP).copied().unwrap_or(0);
        assert_eq!(
            aht, 54,
            "ALLOC_HEAP_TOP should not grow after general free+alloc"
        );
    }

    /// 汎用フリーリスト (>32 セル) の free 後に ALLOC_FREE_HEAD が正しく更新されていることを検証。
    #[test]
    fn test_fsba_reuse_general_freelist_populated() {
        let fsba = FsbaFirstFitAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 40, slot: 0 },
            AllocOp::Free { slot: 0 },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        let afh = heap
            .get(&heap_layout::ALLOC_FREE_HEAD)
            .copied()
            .unwrap_or(0);
        assert_ne!(
            afh, 0,
            "ALLOC_FREE_HEAD should be non-zero after general free"
        );
    }

    /// 異なるサイズクラスの free が互いのフリーリストに影響しないことを検証。
    #[test]
    fn test_fsba_reuse_mixed_class_independent() {
        let fsba = FsbaFirstFitAllocRuntime;
        let ops = vec![
            AllocOp::Alloc { size: 1, slot: 0 }, // class 0
            AllocOp::Alloc { size: 7, slot: 1 }, // class 2
            AllocOp::Free { slot: 0 },
            AllocOp::Free { slot: 1 },
        ];
        let vm = run_alloc_free_sequence(&fsba, 0, &ops);
        let heap = vm.heap();

        let table_ptr = heap.get(&heap_layout::FSBA_TABLE_PTR).copied().unwrap_or(0);
        let class0_head = heap.get(&(table_ptr + 0)).copied().unwrap_or(0);
        let class1_head = heap.get(&(table_ptr + 1)).copied().unwrap_or(0);
        let class2_head = heap.get(&(table_ptr + 2)).copied().unwrap_or(0);

        assert_ne!(class0_head, 0, "Class 0 free list should be populated");
        assert_eq!(class1_head, 0, "Class 1 free list should remain empty");
        assert_ne!(class2_head, 0, "Class 2 free list should be populated");
    }

    /// FSBA プロローグ → エピローグの完全フローをVM上で検証する。
    #[test]
    fn test_fsba_prologue_epilogue_on_vm() {
        use crate::whitespace::{StepResult, WhitespaceVM};

        let fsba = FsbaFirstFitAllocRuntime;
        let mut prog = WsProgram::new();

        // メモリ初期化 (global_heap_size = 2)
        // managed_start = 10, FSBA table at 10-14, AHT = 15
        prog.append(fsba.generate_memory_init(2));

        // 引数を push (arg(0) が深い)
        prog.extend([
            Instruction::Push(WsNumber(42)),
            Instruction::Push(WsNumber(99)),
        ]);

        // プロローグ: local_heap_size=4, arg_offsets=[0, 1]
        prog.append(fsba.generate_function_prologue(4, &[0, 1]));

        // 引数を読み取って出力
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            Instruction::Push(WsNumber(0)),
            Instruction::Add,
            Instruction::Retrieve,
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
            Instruction::Retrieve,
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        // エピローグ
        prog.append(fsba.generate_function_epilogue());

        // LHB が元の値 (8=GLOBAL_PTR) に復元されたことを確認
        prog.extend([
            Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),
            Instruction::Retrieve,
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);

        prog.push(Instruction::Exit);
        prog.append(fsba.generate_subroutines());

        let mut vm = WhitespaceVM::from_instructions(prog.into_instructions())
            .unwrap()
            .with_io(
                Box::new(std::io::Cursor::new(Vec::new())),
                Box::new(Vec::<u8>::new()),
            );
        let result = vm.run(10000);
        assert!(
            matches!(result, StepResult::Complete),
            "VM should exit normally, got: {:?}",
            result
        );
        let output = vm.get_stdout_string();
        // alloc(4) with FSBA: total=5 → class 2 (block_size=8) → bump from AHT=15
        // block=15, ptr=16. LHB set to 16. arg[0]=42 at heap[16], arg[1]=99 at heap[17]
        // After epilogue: LHB restored to 8 (GLOBAL_PTR)
        assert_eq!(output, "42\n99\n8\n");
    }
}
