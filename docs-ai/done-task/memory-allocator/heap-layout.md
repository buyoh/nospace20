# 新しいヒープメモリレイアウト設計

## 現在のレイアウト

```
アドレス  用途
──────── ──────────────────────────────
 -12     EXT_ASSERT_NOT_ADDR (--std-ext debug)
 -11     EXT_ASSERT_ADDR     (--std-ext debug)
 -10     EXT_TRACE_ADDR      (--std-ext debug)
  ...
   0     (未使用)
   1     (予約)
   2     LOCAL_HEAP_BEGIN  ← 現フレーム開始位置
   3     LOCAL_HEAP_END    ← 現フレーム終了位置
   4     TEMP_PTR          ← I/O 一時領域
   5     (予約未使用)
   6     (予約未使用)
   7     (予約未使用)
   8     GLOBAL_PTR        ← グローバル変数領域の開始
   8+G   (static変数領域の開始)
   8+G+S (ローカルヒープの開始 → 右方向にフレーム成長)
```

## `--std-ext alloc` 有効時の新レイアウト

```
アドレス  用途
──────── ──────────────────────────────
 -12     EXT_ASSERT_NOT_ADDR (--std-ext debug)
 -11     EXT_ASSERT_ADDR     (--std-ext debug)
 -10     EXT_TRACE_ADDR      (--std-ext debug)
  ...
   0     (未使用)
   1     (予約)
   2     LOCAL_HEAP_BEGIN  ← 現フレーム開始位置 (変更なし)
   3     (未使用: フレーム管理はアロケータに委譲)
   4     TEMP_PTR          ← I/O 一時領域 (変更なし)
   5     ALLOC_FREE_HEAD   ← 汎用フリーリスト先頭アドレス (新規)
   6     ALLOC_HEAP_TOP    ← マネージドヒープ末尾 (新規)
   7     FSBA_TABLE_PTR    ← FSBA テーブルポインタ (新規)
   8     GLOBAL_PTR        ← グローバル変数領域の開始 (変更なし)
   8+G   (static変数領域の開始) (変更なし)
   8+G+S (FSBAテーブルの開始 → その後マネージドヒープ)
```

### 変更点

| アドレス | 変更前 | 変更後 |
|---|---|---|
| 3 | `LOCAL_HEAP_END` (フレーム終了位置) | 未使用 (アロケータがフレームサイズを管理) |
| 5 | 予約未使用 | `ALLOC_FREE_HEAD` (汎用フリーリスト先頭) |
| 6 | 予約未使用 | `ALLOC_HEAP_TOP` (ヒープ末尾) |
| 7 | 予約未使用 | `FSBA_TABLE_PTR` (FSBA フリーリストテーブルポインタ) |

### `LOCAL_HEAP_END` の廃止

現在の方式では `LOCAL_HEAP_END` が「次のフレーム確保位置」を示す。アロケータ方式では:

- フレームのサイズはアロケータの `alloc(size)` 呼び出しで指定
- フレームの位置はアロケータが決定
- `LOCAL_HEAP_END` は不要になる

ただし `LOCAL_HEAP_BEGIN` は**引き続き必要**。ローカル変数のアドレス解決に `heap[LOCAL_HEAP_BEGIN] + offset` を使う方式は変わらない。

## メモリマップの視覚的表現

### `--std-ext alloc` 無効時 (既存動作、変更なし)

```
low address                                                  high address
┌───────────┬───────────┬──────────┬───────────────────────────────────┐
│ Reserved  │ Globals   │ Statics  │ Local Frames (bump allocation)   │
│ addr 0-7  │ addr 8+   │          │ ← LOCAL_HEAP_END が管理           │
└───────────┴───────────┴──────────┴───────────────────────────────────┘
```

### `--std-ext alloc` 有効時

```
low address                                                  high address
┌───────────┬───────────┬──────────┬────────┬──────────────────────┐
│ Reserved  │ Globals   │ Statics  │ FSBA   │ Managed Heap           │
│ addr 0-7  │ addr 8+   │          │ Table  │ (allocator管理)          │
│ +alloc    │           │          │ (5 cel)│                        │
│ metadata  │           │          │        │ [Frame A][Free][Heap X]│
│ at 5,6,7  │           │          │        │                        │
└───────────┴───────────┴──────────┴────────┴──────────────────────┘
```

マネージドヒープ内はアロケータが自由に配置:
- FSBA テーブル (5 セル): 各サイズクラスのフリーリスト先頭ポインタ
- スタックフレーム (関数呼び出しごとに alloc で確保)
- ユーザー指定のヒープブロック (`__alloc()` で確保)
- フリーブロック (解放済み、FSBA または汎用フリーリストで管理)

## 初期化の変更

### 現在 (`generate_header`)

```
heap[LOCAL_HEAP_BEGIN] = GLOBAL_PTR                  // = 8
heap[LOCAL_HEAP_END]   = GLOBAL_PTR + global_heap_size
```

### `--std-ext alloc` 有効時

```
heap[LOCAL_HEAP_BEGIN] = 0               // 未確保（main 関数呼び出し時に alloc）
heap[ALLOC_FREE_HEAD]  = 0               // 汎用フリーリスト空
heap[ALLOC_HEAP_TOP]   = GLOBAL_PTR + global_heap_size + FSBA_CLASS_COUNT  // FSBAテーブル直後
heap[FSBA_TABLE_PTR]   = GLOBAL_PTR + global_heap_size  // FSBAテーブル先頭
// FSBA テーブル初期化 (5 クラス分)
for i in 0..FSBA_CLASS_COUNT:
    heap[FSBA_TABLE_PTR + i] = 0         // 各サイズクラスのフリーリスト空
```

## memory.rs への変更

### 新しい定数

```rust
impl MemoryLayout {
    // 既存
    pub const LOCAL_HEAP_BEGIN: HeapAddress = HeapAddress(2);
    pub const LOCAL_HEAP_END: HeapAddress = HeapAddress(3);   // alloc 時は未使用
    pub const TEMP_PTR: HeapAddress = HeapAddress(4);
    pub const GLOBAL_PTR: HeapAddress = HeapAddress(8);

    // 新規 (--std-ext alloc)
    pub const ALLOC_FREE_HEAD: HeapAddress = HeapAddress(5);
    pub const ALLOC_HEAP_TOP: HeapAddress = HeapAddress(6);
    pub const FSBA_TABLE_PTR: HeapAddress = HeapAddress(7);
}
```

### heap_layout への追加

```rust
pub mod heap_layout {
    // 既存
    pub const LOCAL_HEAP_BEGIN: i64 = 2;
    pub const LOCAL_HEAP_END: i64 = 3;
    pub const TEMP_PTR: i64 = 4;
    pub const GLOBAL_PTR: i64 = 8;

    // 新規 (--std-ext alloc)
    pub const ALLOC_FREE_HEAD: i64 = 5;
    pub const ALLOC_HEAP_TOP: i64 = 6;
    pub const FSBA_TABLE_PTR: i64 = 7;

    // FSBA サイズクラス数
    pub const FSBA_CLASS_COUNT: i64 = 5;
    // FSBA サイズクラス: [2, 4, 8, 16, 32]
}
```
