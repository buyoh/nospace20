# InterpreterAllocator のデータ構造とアルゴリズム

## データ構造

### MemoryBlock

```rust
/// 単一のメモリブロック
struct MemoryBlock {
    /// ブロックのデータ（実際のメモリ）
    data: Vec<i64>,
    /// 解放済みフラグ
    is_freed: bool,
}
```

### InterpreterAllocator

```rust
use std::collections::BTreeMap;

/// インタプリタ用メモリアロケータ
///
/// 仮想1次元アドレス空間を管理する。
/// 各アロケーションは独立した Vec<i64> で保持され、
/// BTreeMap で仮想アドレスから実際のブロックへマッピングされる。
pub(crate) struct InterpreterAllocator {
    /// 開始アドレス → メモリブロック のマッピング
    blocks: BTreeMap<i64, MemoryBlock>,
    /// 次に割り当てる仮想アドレス（バンプポインタ）
    next_addr: i64,
}
```

## アルゴリズム

### alloc(size)

```
fn alloc(&mut self, size: usize) -> i64:
    if size == 0:
        // サイズ 0 は最小 1 として扱う（アドレスの一意性を保証）
        return alloc(1)

    addr = self.next_addr
    self.next_addr += size as i64

    block = MemoryBlock {
        data: vec![0; size],
        is_freed: false,
    }
    self.blocks.insert(addr, block)

    return addr
```

- 常に `next_addr` からバンプ割り当て
- 解放済みブロックの再利用は行わない（シンプルさ優先）
- `size == 0` の場合は `size = 1` として扱い、有効なアドレスを返す

### free(addr)

```
fn free(&mut self, addr: i64):
    block = self.blocks.get_mut(addr)
    if block is None:
        panic!("runtime error: free: invalid address {addr}")
    if block.is_freed:
        panic!("runtime error: double free at address {addr}")

    block.is_freed = true
```

- ブロックの開始アドレスと正確に一致する必要がある
- 二重解放は実行時エラー
- ブロック自体は BTreeMap に残す（アドレス空間の管理のため）

### get(addr)

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

### set(addr, value)

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
アロケータの `alloc` はデフォルトで 0 初期化するが、`alloc_uninit` メソッドを追加し、
`randomize_uninit` モード時にはこちらを使用する。

```rust
/// 未初期化（0 or ランダム値）でメモリを確保
fn alloc_uninit(&mut self, size: usize, randomize: bool) -> i64:
    addr = alloc(size)
    if randomize:
        for i in 0..size:
            block.data[i] = random_uninit_value()
    return addr
```

## 複雑度分析

| 操作 | 時間計算量 |
|------|-----------|
| `alloc(n)` | O(n) （Vec 初期化） |
| `free(addr)` | O(log B) （B: ブロック数） |
| `get(addr)` | O(log B) |
| `set(addr, value)` | O(log B) |

Whitespace コンパイラの FSBA (O(1)) と比べると遅いが、
インタプリタの主目的はテスト・検証であり、十分な性能。

## エラーメッセージ

| エラー条件 | メッセージ |
|---|---|
| 未割当アドレスの読み取り/書き込み | `"runtime error: invalid memory access at address {addr}"` |
| 解放済みアドレスの読み取り/書き込み | `"runtime error: access to freed memory at address {addr}"` |
| 無効アドレスの free | `"runtime error: free: invalid address {addr}"` |
| 二重 free | `"runtime error: double free at address {addr}"` |
| ブロック途中のアドレスを free | `"runtime error: free: address {addr} is not a block start (block starts at {block_start})"` |
