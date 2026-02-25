# メモリアロケータ再利用効率テスト

## 概要

メモリアロケータが解放された領域を効率よく再利用しているかを検証するユニットテストを追加する。

### 背景

現在の分離テスト (L1) および E2E テスト (L3) は「確保・解放後にデータが正しく書き込める」ことを確認しているが、**解放された領域が実際に再利用されている（ヒープが無制限に成長していない）** ことは検証していない。

再利用の検証において「同じアドレスが返される」かどうかは実装依存であり、テストの前提にすべきではない。代わりに、VM のヒープ状態を直接検査することで、実装ごとに適切な再利用を検証できる。

### 方針

- **分離テスト (L1) / E2E テスト (L3) ではなく、ユニットテスト (L2) で検証する**
- `WhitespaceVM::heap()` を使ってヒープメタデータを直接検査する
- 各 `AllocRuntime` 実装ごとに、その実装の特性に基づいたテストを書く

## 検証手法

### ヒープ状態の直接検査

`WhitespaceVM` は `heap() -> &HashMap<i64, i64>` メソッドを公開しており、VM 実行後にヒープのメタデータを直接読み取れる。

検査対象:

| アドレス | 定数名 | 内容 |
|---|---|---|
| 3 | `LOCAL_HEAP_END` | バンプポインタ末尾（BumpAllocRuntime） |
| 5 | `ALLOC_FREE_HEAD` | 汎用フリーリスト先頭（FSBA） |
| 6 | `ALLOC_HEAP_TOP` | マネージドヒープ末尾（FSBA） |
| 7 | `FSBA_TABLE_PTR` | FSBA テーブルポインタ（FSBA） |
| `TABLE + i` | — | 各サイズクラスのフリーリスト先頭（FSBA） |

### テストパターン

各テストは以下の共通パターンで構成する:

1. アロケータの `generate_memory_init` + `generate_subroutines` で VM を構築
2. WS 命令列で `__rt_alloc` / `__rt_free` を呼び出す操作を構築
3. `vm.run()` で実行
4. `vm.heap()` でヒープメタデータを検査
5. アサーション: 再利用が行われていればヒープ末尾ポインタが成長しない、フリーリストが正しく更新されるなど

### ヘルパー関数

テストの共通パターンを関数化する:

```rust
/// alloc/free 操作列を受け取り、実行後のVMを返すヘルパー
fn run_alloc_free_sequence(
    runtime: &dyn AllocRuntime,
    global_heap_size: i64,
    ops: &[AllocOp],
) -> WhitespaceVM { ... }

enum AllocOp {
    /// __rt_alloc(size), 結果のポインタをヒープ上の slot に保存
    Alloc { size: i64, slot: i64 },
    /// __rt_free(heap[slot])
    Free { slot: i64 },
}
```

> 実装の詳細はテストコード作成時に最適化してよい。ポイントは `alloc` で返されたポインタを変数としてヒープに保存し、`free` でそれを読み取って解放する一連のパターンをシンプルに記述できることである。

## テストケース設計

### BumpAllocRuntime

#### `test_bump_reuse_lifo_heap_stable`

LIFO 順で free→alloc したとき、`LOCAL_HEAP_END` が成長しないことを検証。

```
操作: alloc(3) → alloc(2) → free(ptr2) → alloc(2)
検査: free(ptr2) 後の LOCAL_HEAP_END == 再alloc(2) 後の LOCAL_HEAP_END
```

> BumpAllocRuntime の `__rt_free(ptr)` は `LOCAL_HEAP_END = ptr` に設定するため、LIFO 順の free では末尾が巻き戻る。

#### `test_bump_reuse_loop_heap_stable`

ループでの alloc/free（LIFO 順）で `LOCAL_HEAP_END` が一定に保たれることを検証。

```
操作: alloc(A) → [alloc(B) → free(ptrB)] × N回 → 最終 LOCAL_HEAP_END 検査
検査: ループ前後で LOCAL_HEAP_END が同一
```

### FsbaFirstFitAllocRuntime

#### `test_fsba_reuse_class0_heap_stable`

class 0 (サイズ 1) の alloc→free→alloc で `ALLOC_HEAP_TOP` が成長しないことを検証。

```
操作: alloc(1) → free(ptr) → alloc(1)
検査: 2回目 alloc 後の ALLOC_HEAP_TOP == 1回目 alloc 後の ALLOC_HEAP_TOP
```

#### `test_fsba_reuse_each_class_heap_stable`

各サイズクラス (0-4) について alloc→free→alloc で `ALLOC_HEAP_TOP` が成長しないことをパラメトリックに検証。

```
サイズクラス 0: alloc(1) → free → alloc(1)
サイズクラス 1: alloc(3) → free → alloc(3)
サイズクラス 2: alloc(7) → free → alloc(7)
サイズクラス 3: alloc(15) → free → alloc(15)
サイズクラス 4: alloc(31) → free → alloc(31)
検査: 各クラスで 2回目 alloc 後の ALLOC_HEAP_TOP == 1回目 alloc 後の ALLOC_HEAP_TOP
```

#### `test_fsba_reuse_roundup_heap_stable`

異なるリクエストサイズでも同一サイズクラスに切り上げられる場合、free→alloc で再利用されることを検証。

```
操作: alloc(2) → free → alloc(3) (both → class 1, block size 4)
検査: ALLOC_HEAP_TOP が成長しない
```

#### `test_fsba_reuse_loop_heap_stable`

100 回の alloc/free ループで `ALLOC_HEAP_TOP` が一定に保たれることを検証。

```
操作: [alloc(3) → free] × 100回
検査: 1回目 alloc 後の ALLOC_HEAP_TOP == 100回目 free 後の ALLOC_HEAP_TOP
```

#### `test_fsba_reuse_freelist_populated`

free 後にフリーリストが正しく更新されていることを直接検証。

```
操作: alloc(1) → free(ptr)
検査:
  - FSBA テーブルの class 0 エントリ != 0（フリーリストに要素がある）
```

#### `test_fsba_reuse_freelist_empty_after_realloc`

free→alloc でフリーリストからポップされ、リストが空に戻ることを検証。

```
操作: alloc(1) → free(ptr) → alloc(1)
検査:
  - FSBA テーブルの class 0 エントリ == 0（フリーリストが空に戻った）
```

#### `test_fsba_reuse_general_heap_stable`

32 セル超（汎用アロケータ経由）の alloc→free→alloc で `ALLOC_HEAP_TOP` が成長しないことを検証。

```
操作: alloc(40) → free(ptr) → alloc(40)
検査: 2回目 alloc 後の ALLOC_HEAP_TOP == 1回目 alloc 後の ALLOC_HEAP_TOP
```

#### `test_fsba_reuse_general_freelist_populated`

汎用フリーリスト (>32 セル) の free 後に `ALLOC_FREE_HEAD` が正しく更新されていることを検証。

```
操作: alloc(40) → free(ptr)
検査: ALLOC_FREE_HEAD != 0（汎用フリーリストに要素がある）
```

#### `test_fsba_reuse_mixed_class_independent`

異なるサイズクラスの free が互いのフリーリストに影響しないことを検証。

```
操作: alloc(1) [class0], alloc(7) [class2] → free(ptr_class0) → free(ptr_class2)
検査:
  - FSBA テーブルの class 0 エントリ != 0
  - FSBA テーブルの class 1 エントリ == 0（未使用クラスは空のまま）
  - FSBA テーブルの class 2 エントリ != 0
```

## テスト一覧

| # | テスト名 | 実装 | 検証内容 |
|---|---|---|---|
| 1 | `test_bump_reuse_lifo_heap_stable` | Bump | LIFO free で LOCAL_HEAP_END 巻き戻し |
| 2 | `test_bump_reuse_loop_heap_stable` | Bump | ループ alloc/free で LOCAL_HEAP_END 一定 |
| 3 | `test_fsba_reuse_class0_heap_stable` | FSBA | class 0 再利用で ALLOC_HEAP_TOP 不変 |
| 4 | `test_fsba_reuse_each_class_heap_stable` | FSBA | 全クラスで ALLOC_HEAP_TOP 不変 |
| 5 | `test_fsba_reuse_roundup_heap_stable` | FSBA | サイズ切り上げでの再利用 |
| 6 | `test_fsba_reuse_loop_heap_stable` | FSBA | 100 回ループで ALLOC_HEAP_TOP 一定 |
| 7 | `test_fsba_reuse_freelist_populated` | FSBA | free 後のフリーリスト状態 |
| 8 | `test_fsba_reuse_freelist_empty_after_realloc` | FSBA | 再 alloc 後のフリーリスト空 |
| 9 | `test_fsba_reuse_general_heap_stable` | FSBA | 汎用アロケータの再利用 |
| 10 | `test_fsba_reuse_general_freelist_populated` | FSBA | 汎用フリーリストの状態 |
| 11 | `test_fsba_reuse_mixed_class_independent` | FSBA | クラス間フリーリスト独立性 |

## 実装場所

`src/compiler_ws/alloc_runtime.rs` 内の `#[cfg(test)] mod tests` に追加する。

既存のテスト (`test_bump_alloc_free_on_vm`, `test_fsba_alloc_free_reuse_on_vm` 等) と同じパターンで、VM を構築・実行し、`vm.heap()` でメタデータを検査する。

## 実装計画

### 単一フェーズ

1. ヘルパー関数の実装（WS 命令列構築の共通化）
2. BumpAllocRuntime テスト 2 件追加
3. FsbaFirstFitAllocRuntime テスト 9 件追加
4. `cargo test` で全テストパス確認
