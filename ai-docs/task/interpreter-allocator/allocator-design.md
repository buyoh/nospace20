# InterpreterAllocator のデータ構造とアルゴリズム

## データ構造

### MemoryBlock

```rust
/// 単一のメモリブロック
struct MemoryBlock {
    /// ブロックのデータ（実際のメモリ）
    /// ヘッダー（block[0]=size）を含む合計サイズ分の配列。
    /// ユーザーがアクセスできるのは data[1..] (= ptr 以降)。
    data: Vec<i64>,
    /// 解放済みフラグ
    is_freed: bool,
}
```

### InterpreterAllocator

```rust
use std::collections::BTreeMap;
use crate::algorithm::alloc_spec;

/// インタプリタ用メモリアロケータ
///
/// WS コンパイラと同一の FSBA + First-Fit + バンプ アルゴリズムを
/// Rust で直接実行する。アルゴリズム定数は `alloc_spec` から参照。
///
/// 仮想1次元アドレス空間を管理する。
/// 各アロケーションは独立した Vec<i64> で保持され、
/// BTreeMap で仮想アドレスから実際のブロックへマッピングされる。
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
```

### アドレスモデル

WS コンパイラと同じブロック構造:

```
block (ブロック開始アドレス):
  block[0] = total_size       ← ヘッダー (alloc_spec::HEADER_SIZE = 1)
  block[1] = next_free_ptr    ← フリーリスト繋ぎ先（使用中は未定義）
  block[1..total_size]        ← ユーザーデータ
  ^
  ptr (= block + 1) がユーザーに返すアドレス
```

- `data: Vec<i64>` のインデックス 0 がヘッダー（`total_size`）
- ユーザーに返す `ptr` = ブロック開始アドレス + 1
- フリーリストでは `data[1]` に次のフリーブロックのアドレスを格納

## アルゴリズム

`alloc_spec` の定数・関数を使用する（[algorithm-separation.md](algorithm-separation.md) を参照）。

### alloc(user_size) — `__alloc` 用

WS コンパイラの FSBA + First-Fit + バンプ と同一ロジック。

```
fn alloc(&mut self, user_size: i64) -> i64:
    total = alloc_spec::total_from_user_size(user_size)
    // = max(user_size + HEADER_SIZE, MIN_BLOCK_SIZE)

    match alloc_spec::fsba_class_for(total):
        Some(class) => return self.fsba_alloc(class)
        None        => return self.general_alloc(total)
```

#### fsba_alloc(class_index)

```
fn fsba_alloc(&mut self, class: usize) -> i64:
    class_size = alloc_spec::FSBA_BLOCK_SIZES[class]
    free_head = self.fsba_free_lists[class]

    if free_head != 0:
        // フリーリストからポップ
        block = self.blocks.get_mut(free_head)
        block.is_freed = false
        next = block.data[1]  // next_free_ptr
        self.fsba_free_lists[class] = next
        return free_head + 1  // ptr = block + 1

    // フリーリスト空 → バンプ割り当て
    return self.bump_alloc(class_size)
```

#### general_alloc(total)

```
fn general_alloc(&mut self, total: i64) -> i64:
    // First-Fit 探索
    prev_next_ref = &mut self.general_free_head
    curr = *prev_next_ref

    while curr != 0:
        block = self.blocks.get(curr)
        curr_size = block.data[0]

        if curr_size >= total:
            // ブロック発見
            if alloc_spec::can_split(curr_size, total):
                // 分割: 前半を使用、後半をフリーリストに残す
                remainder_addr = curr + total
                remainder_size = curr_size - total

                // 残余ブロック作成
                remainder_block = MemoryBlock {
                    data: vec![0; remainder_size],
                    is_freed: false,   // フリーリスト上だが block として存在
                }
                remainder_block.data[0] = remainder_size
                remainder_block.data[1] = block.data[1]  // next pointer 継承
                self.blocks.insert(remainder_addr, remainder_block)

                // 現ブロックを縮小
                block.data[0] = total
                block.data.truncate(total)
                block.is_freed = false
                *prev_next_ref = remainder_addr

                return curr + 1

            else:
                // 分割不可: ブロック全体を使用
                block.is_freed = false
                *prev_next_ref = block.data[1]  // リストから除去
                return curr + 1

        // 次のブロックへ
        prev_next_ref = &mut block.data[1]  // 概念上: next pointer の参照
        curr = block.data[1]

    // 適合ブロックなし → バンプ割り当て
    return self.bump_alloc(total)
```

#### bump_alloc(total)

```
fn bump_alloc(&mut self, total: i64) -> i64:
    addr = self.next_addr
    self.next_addr += total

    block = MemoryBlock {
        data: vec![0; total as usize],
        is_freed: false,
    }
    block.data[0] = total  // ヘッダー: ブロック合計サイズ
    self.blocks.insert(addr, block)

    return addr + 1  // ptr = block + 1
```

### free(ptr) — `__free` 用

```
fn free(&mut self, ptr: i64):
    block_addr = ptr - 1
    block = self.blocks.get_mut(block_addr)

    if block is None:
        panic!("runtime error: free: invalid address {ptr}")
    if block.is_freed:
        panic!("runtime error: double free at address {ptr}")

    block_size = block.data[0]
    block.is_freed = true

    match alloc_spec::fsba_class_for(block_size):
        Some(class) => self.fsba_free(block_addr, class)
        None        => self.general_free(block_addr)
```

#### fsba_free(block_addr, class)

```
fn fsba_free(&mut self, block_addr: i64, class: usize):
    block = self.blocks.get_mut(block_addr)
    block.data[1] = self.fsba_free_lists[class]  // old head → next
    self.fsba_free_lists[class] = block_addr
```

#### general_free(block_addr)

```
fn general_free(&mut self, block_addr: i64):
    block = self.blocks.get_mut(block_addr)
    block.data[1] = self.general_free_head  // old head → next
    self.general_free_head = block_addr
```

### alloc_internal(size) — スコープ・グローバル変数用

内部用の簡易割り当て。FSBA/First-Fit は不要（スコープは LIFO で解放され再利用しない）。

```
fn alloc_internal(&mut self, size: usize) -> i64:
    if size == 0:
        return alloc_internal(1)

    addr = self.next_addr
    self.next_addr += size as i64

    block = MemoryBlock {
        data: vec![0; size],
        is_freed: false,
    }
    self.blocks.insert(addr, block)

    return addr
```

- ヘッダーなし（FSBA 管理外）
- 返すアドレスがそのままベースアドレス（+1 しない）
- `free_internal` は `is_freed = true` にするだけ（フリーリストに返さない）

### get(addr) / set(addr, value)

```
fn get(&self, addr: i64) -> i64:
    (block_start, block) = find_block_containing(addr)
    if block is None:
        panic!("runtime error: invalid memory access at address {addr}")
    if block.is_freed:
        panic!("runtime error: access to freed memory at address {addr}")

    offset = (addr - block_start) as usize
    return block.data[offset]
```

```
fn set(&mut self, addr: i64, value: i64):
    (block_start, block) = find_block_containing_mut(addr)
    if block is None:
        panic!("runtime error: invalid memory access at address {addr}")
    if block.is_freed:
        panic!("runtime error: access to freed memory at address {addr}")

    offset = (addr - block_start) as usize
    block.data[offset] = value
```

### find_block_containing(addr)

```
fn find_block_containing(&self, addr: i64) -> Option<(i64, &MemoryBlock)>:
    // BTreeMap::range(..=addr) で addr 以下の最大のキーを見つける
    entry = self.blocks.range(..=addr).next_back()
    if entry is None:
        return None

    (block_start, block) = entry
    // アドレスがブロック範囲内かチェック
    if addr >= block_start + block.data.len() as i64:
        return None

    return Some((block_start, block))
```

- O(log n) でアドレスからブロックを検索
- `BTreeMap::range(..=addr).next_back()` で addr 以下の最大キーを取得
- ブロックの範囲内かを確認

## randomize_uninit 対応

現在 `create_uninit_vec` で `randomize_uninit` モードの場合にランダム値で初期化している。
`alloc_internal` はデフォルトで 0 初期化するが、`alloc_internal_uninit` を追加し、
`randomize_uninit` モード時にはこちらを使用する。

```rust
/// 未初期化（0 or ランダム値）で内部メモリを確保
fn alloc_internal_uninit(&mut self, size: usize, randomize: bool) -> i64:
    addr = alloc_internal(size)
    if randomize:
        block = self.blocks.get_mut(addr)
        for i in 0..size:
            block.data[i] = random_uninit_value()
    return addr
```

## 複雑度分析

| 操作 | 時間計算量 |
|------|-----------|
| `alloc(n)` (FSBA, フリーリスト有) | O(1) + O(log B) |
| `alloc(n)` (バンプ) | O(n) (Vec 初期化) + O(log B) |
| `alloc(n)` (General, First-Fit) | O(F) (F: フリーリスト長) + O(log B) |
| `free(ptr)` | O(log B) |
| `get(addr)` | O(log B) |
| `set(addr, value)` | O(log B) |
| `alloc_internal(n)` | O(n) + O(log B) |

B はブロック数。BTreeMap のルックアップが O(log B)。
WS コンパイラの FSBA (純粋な O(1)) と比べるとオーバーヘッドがあるが、
インタプリタの主目的はテスト・検証であり、十分な性能。

## エラーメッセージ

| エラー条件 | メッセージ |
|---|---|
| 未割当アドレスの読み取り/書き込み | `"runtime error: invalid memory access at address {addr}"` |
| 解放済みアドレスの読み取り/書き込み | `"runtime error: access to freed memory at address {addr}"` |
| 無効アドレスの free | `"runtime error: free: invalid address {addr}"` |
| 二重 free | `"runtime error: double free at address {addr}"` |
| ブロック途中のアドレスを free | `"runtime error: free: address {addr} is not a block start (block starts at {block_start})"` |
