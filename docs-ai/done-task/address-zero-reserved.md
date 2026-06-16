# アドレス 0 の予約仕様追加

## 概要

`__alloc` や変数アドレスがアドレス 0 を返さないことを言語仕様として明文化し、
実装がその保証を満たしているか確認・修正する。

## 背景

アドレス 0 を「無効アドレス」（ヌルポインタ相当）として使えるようにしたい。
現在の実装では暗黙的にアドレス 0 が使われないようになっているが、仕様として明文化されていない。

## 現状調査結果

### WSコンパイラ

| 項目 | 結果 | 詳細 |
|------|------|------|
| 予約アドレス | 0, 1 は未使用 | `LOCAL_HEAP_BEGIN=2`, `GLOBAL_PTR=8` |
| グローバル変数 | アドレス 8 以降 | `GLOBAL_PTR(8) + offset` |
| ローカル変数 | グローバル領域の後ろ | `heap[LOCAL_HEAP_BEGIN] + offset` |
| BumpAlloc `__rt_alloc` | 最小で 8 を返す | `heap[LOCAL_HEAP_END]` から開始、初期値は `GLOBAL_PTR + global_heap_size` |
| FSBA `__rt_alloc` | 最小で 14 を返す | `managed_start + FSBA_CLASS_COUNT + 1` |
| フリーリスト | 0 をセンチネルとして使用 | `ALLOC_FREE_HEAD=0`, FSBAテーブル各エントリ=0 で「空」を表現 |

### インタプリタ

| 項目 | 結果 | 詳細 |
|------|------|------|
| `next_addr` 初期値 | **1** | コメント: "0 はフリーリストのセンチネル値" |
| `alloc(size)` | 最小で 2 を返す | `block_addr=1`, `ptr = block_addr + 1 = 2` |
| `alloc_internal(size)` | 最小で 1 を返す | `next_addr=1` から開始 |
| `global_base_addr` | 最小で 1 | `alloc_internal_uninit` の最初の呼び出し結果 |
| グローバル変数 | 最小で 1 | `global_base_addr + local_index` |
| フリーリスト | 0 をセンチネルとして使用 | `fsba_free_lists=[0; 5]`, `general_free_head=0` |

### 結論

**すべてのアロケータ・変数アドレスにおいて、アドレス 0 が返されることはない。**

- WSコンパイラ: アドレス 0, 1 は元々未使用。最小のユーザー可視アドレスは 8（グローバル変数）
- インタプリタ: `next_addr=1` から開始するため、アドレス 0 は割り当てられない。最小のユーザー可視アドレスは 1（グローバル変数）
- 両方とも、FSBA/汎用フリーリストでアドレス 0 を「空リスト」のセンチネルとして暗黙的に使用している

## 仕様変更案

### 言語仕様 (`docs/spec.md`) への追記

`__alloc` のセクションに以下の仕様を追加:

> - `__alloc(n)` は **0 以外** のアドレスを返す。アドレス 0 は無効アドレスとして予約されており、変数や `__alloc` が返すアドレスとして使用されることはない。

具体的な追記箇所: `docs/spec.md` のメモリ管理組み込み関数セクション（L828 付近）

### 追記案テキスト

現在:

```
| `__alloc(n)` | n ワード分のヒープメモリを確保し、先頭アドレスを返す |
| `__free(ptr)` | `__alloc` で確保したメモリを解放し、0 を返す |
```

変更後:

```
| `__alloc(n)` | n ワード分のヒープメモリを確保し、先頭アドレスを返す。返すアドレスは常に 0 以外である |
| `__free(ptr)` | `__alloc` で確保したメモリを解放し、0 を返す |
```

また、セクション末尾に注記を追加:

```
**アドレス 0 の予約**: アドレス 0 は無効アドレスとして予約されている。
`__alloc` が返すアドレス、グローバル変数のアドレス、ローカル変数のアドレスのいずれも 0 になることはない。
この性質を利用して、ポインタが有効かどうかの判定に 0 との比較を用いることができる。
```

## 実装への影響

### 変更不要な箇所

現在の実装はすべてこの仕様を満たしている:

1. **WSコンパイラ BumpAllocRuntime** — `__rt_alloc` は `heap[LOCAL_HEAP_END]` を返す。初期値は `GLOBAL_PTR + global_heap_size >= 8`。変更不要。
2. **WSコンパイラ FsbaFirstFitAllocRuntime** — バンプ拡張で `heap[ALLOC_HEAP_TOP]` を返す。初期値は `managed_start + FSBA_CLASS_COUNT >= 13`。ユーザーポインタは `+1` されるので最小 14。変更不要。
3. **インタプリタ InterpreterAllocator** — `next_addr = 1` から開始。`alloc()` は最小 2、`alloc_internal()` は最小 1 を返す。変更不要。
4. **グローバル変数アドレス（WSコンパイラ）** — `GLOBAL_PTR = 8` から開始。変更不要。
5. **グローバル変数アドレス（インタプリタ）** — `alloc_internal` の結果（最小 1）。変更不要。

### 推奨: ドキュメント・コメント追加

- `src/compiler_ws/memory.rs` のメモリマップコメントにアドレス 0 の予約を明記
- `src/interpreter/allocator.rs` のコメントにアドレス 0 が返されない保証を明記
- `docs-ai/spec/compiler-rust-impl/memory-label.md` のメモリマップに反映

### 任意: アサーションの追加

実装の保証を強化するため、以下のアサーションを追加してもよい:

- `InterpreterAllocator::alloc()` と `alloc_internal()` の戻り値が 0 でないことを `debug_assert!`
- WSコンパイラ側は WS コード生成なので静的なアサーションは不要（テストで保証）

## 作業ステップ

1. `docs/spec.md` に仕様追記 ✅
   - `__alloc(n)` の説明に「返すアドレスは常に 0 以外である」を追記
   - アドレス 0 の予約注記を追加（アドレス 0 への書き込みは未定義動作の旨も明記）
2. `src/compiler_ws/memory.rs` のコメント更新 ✅
   - 予約アドレスセクションにアドレス 0 が無効アドレスとして予約されている旨を追記
3. `src/interpreter/allocator.rs` のコメント更新 ✅
   - `new()` のコメントを更新し、alloc/alloc_internal が 0 を返さない保証を明記
4. `docs-ai/spec/compiler-rust-impl/memory-label.md` のメモリマップ更新 ✅
   - アドレス 0 の予約・未定義動作を明記
5. `debug_assert!` の追加 ✅
   - `InterpreterAllocator::alloc()` と `alloc_internal()` の戻り値が 0 でないことを `debug_assert!`
6. テストケース追加 ✅
   - `resources/tests/passes/builtins/alloc_nonzero_001.ns` を追加
   - `test-manifest.yaml` に `test_builtin_alloc_nonzero_001` を登録
   - テスト結果: 4/4 通過
