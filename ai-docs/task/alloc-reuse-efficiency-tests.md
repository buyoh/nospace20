# メモリアロケータ再利用効率テスト

## 概要

メモリアロケータが解放された領域を効率よく再利用しているかを検証するテストを追加する。

現在の分離テスト (L1) および E2E テスト (L3) には「再確保後にデータが正しく書き込める」ことを確認するテストは存在するが、**解放されたメモリが実際に再利用されている（同じアドレスが返される）こと**を直接検証するテストがない。

### 現状の不足

| 既存テスト | 検証内容 | 不足点 |
|---|---|---|
| `alloc_free_reuse_001` (basic) | free→再alloc後にデータ書き込み可 | 同一アドレスかは未確認 |
| `fsba_class_reuse_001` | 同一クラス free→alloc | 同一アドレスかは未確認 |
| `fsba_free_reuse_002` | 異サイズ free→alloc | 再利用されたかは未確認 |
| `fsba_repeated_001` | 100回ループ | ヒープ成長は未確認 |

### 目標

1. **ポインタ再利用の検証**: free 後の alloc で同じアドレスが返されることを直接確認
2. **ヒープ非成長の検証**: free→alloc サイクルで `ALLOC_HEAP_TOP` が増加しないことを確認
3. **LIFO 順序の検証**: FSBA フリーリストが LIFO (後入れ先出し) 順で再利用されることを確認
4. **全サイズクラスの網羅**: FSBA の 5 クラス (2, 4, 8, 16, 32 セル) すべてで再利用を検証
5. **汎用アロケータの再利用**: 32 セル超ブロックの First-Fit 再利用を検証

## 必要なインフラ変更

### 新規操作: `assert_var_eq`

既存の `assert_var_ne`（2 変数が異なることの検証）の逆で、2 変数が**等しい**ことを検証する操作を追加する。

```json
{ "op": "assert_var_eq", "var1": "p1", "var2": "p2" }
```

`heap[var1_addr] != heap[var2_addr]` ならば `__test_fail` へジャンプしテスト失敗。

#### WS 変換

```
push <var1_heap_addr>
retrieve
push <var2_heap_addr>
retrieve
sub                    ; diff = var1 - var2
jz __skip_{n}          ; if 0 (equal) → OK, skip
jmp __test_fail        ; not equal → fail
Label(__skip_{n}):
```

#### 実装箇所

- `tests/alloc_test.rs` の `AllocStep` enum に `AssertVarEq` バリアントを追加
- `MiniCompiler::compile_step` に分岐追加
- `MiniCompiler::compile_assert_var_eq` メソッドを新規実装
- 条件分岐の「成功時スキップ」用にラベル ID を動的割り当て（`self.alloc_label()` を使用）

### `heap_print` の活用

`ALLOC_HEAP_TOP`（アドレス 6）を `heap_print` で出力し、期待値と完全一致で検証する。これにより、free → alloc 後にヒープが成長していないことを確認できる。新規操作の追加は不要。

## テストケース設計

### L1: 分離テスト（JSON ミニ言語）

#### カテゴリ: `reuse/` — 再利用効率テスト

新しいサブディレクトリ `resources/tests_alloc/reuse/` を作成する。

##### 1. `reuse_ptr_class0_001` — クラス 0 ポインタ再利用

alloc(1)→free→alloc(1) で同じポインタが返されることを検証。

```json
{
  "description": "FSBA class 0: alloc(1)→free→alloc(1) で同じポインタが返される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 1 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 1 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "2\n"
  }
}
```

> p1 に alloc(1) → counter=[1], counter→2。free(p1)。p2 に alloc(1) → p1 と同じアドレス (FSBA class 0 フリーリストから再利用)。counter=[2], counter→3。assert p1==p2。

##### 2. `reuse_ptr_class1_001` — クラス 1 ポインタ再利用

```json
{
  "description": "FSBA class 1: alloc(3)→free→alloc(3) で同じポインタが返される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 3 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 3 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "4\n5\n6\n"
  }
}
```

##### 3. `reuse_ptr_class2_001` — クラス 2 ポインタ再利用

```json
{
  "description": "FSBA class 2: alloc(7)→free→alloc(7) で同じポインタが返される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 7 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 7 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "8\n9\n10\n11\n12\n13\n14\n"
  }
}
```

##### 4. `reuse_ptr_class3_001` — クラス 3 ポインタ再利用

```json
{
  "description": "FSBA class 3: alloc(15)→free→alloc(15) で同じポインタが返される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 15 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 15 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
  }
}
```

##### 5. `reuse_ptr_class4_001` — クラス 4 ポインタ再利用

```json
{
  "description": "FSBA class 4: alloc(31)→free→alloc(31) で同じポインタが返される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 31 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 31 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n"
  }
}
```

##### 6. `reuse_heap_stable_001` — ヒープ非成長の検証

alloc→free→alloc 後に `ALLOC_HEAP_TOP` が変化しないことを確認。

```json
{
  "description": "FSBA: alloc→free→alloc で ALLOC_HEAP_TOP が成長しない",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 3 },
    { "op": "heap_print", "address": 6 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 3 },
    { "op": "heap_print", "address": 6 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "print", "value": 999 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "?"
  }
}
```

> `ALLOC_HEAP_TOP` の具体的な値は実装時にヒープレイアウトから計算する。2 回の `heap_print` で同じ値が出力されることがポイント。末尾の `print 999` は正常完了マーカー。

ヒープレイアウト計算:
- GLOBAL_PTR = 8, global_heap_size = 0, vars = 2
- effective_global_size = 0 + 1(counter) + 2(vars) = 3
- FSBA テーブル開始: 8 + 3 = 11
- ALLOC_HEAP_TOP 初期値: 11 + 5(FSBA_CLASS_COUNT) = 16
- alloc(3): total = 4 → class 1 (block size 4), 新規割当 → ALLOC_HEAP_TOP = 16 + 4 = 20
- free(p1): CLASS 1 フリーリストにプッシュ → ALLOC_HEAP_TOP 変化なし = 20
- alloc(3): total = 4 → class 1, フリーリストヒット → ALLOC_HEAP_TOP 変化なし = 20

期待 stdout: `"20\n20\n999\n"`

##### 7. `reuse_lifo_order_001` — LIFO 順序の検証

複数ブロックを free した後、LIFO 順で再利用されることを検証。

```json
{
  "description": "FSBA: LIFO 順序で再利用 — 後に free したブロックが先に alloc される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2", "p3", "q1", "q2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 1 },
    { "op": "alloc", "var": "p2", "size": 1 },
    { "op": "alloc", "var": "p3", "size": 1 },
    { "op": "free", "var": "p1" },
    { "op": "free", "var": "p2" },
    { "op": "free", "var": "p3" },
    { "op": "alloc", "var": "q1", "size": 1 },
    { "op": "alloc", "var": "q2", "size": 1 },
    { "op": "assert_var_eq", "var1": "q1", "var2": "p3" },
    { "op": "assert_var_eq", "var1": "q2", "var2": "p2" },
    { "op": "print", "value": 888 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "888\n"
  }
}
```

> FSBA フリーリストは LIFO（スタック）方式。p1, p2, p3 の順で free すると、フリーリスト先頭は p3→p2→p1 の順。次の alloc は p3 から返される。

##### 8. `reuse_loop_stable_001` — ループでのヒープ安定性

100 回の alloc/free ループ後に ALLOC_HEAP_TOP が変化しないことを検証。

```json
{
  "description": "FSBA: 100 回 alloc/free ループで ALLOC_HEAP_TOP が一定",
  "config": { "allocator": "fsba", "max_steps": 1000000 },
  "vars": ["p1"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 3 },
    { "op": "free", "var": "p1" },
    { "op": "heap_print", "address": 6 },
    { "op": "loop", "count": 99, "body": [
      { "op": "alloc", "var": "p1", "size": 3 },
      { "op": "free", "var": "p1" }
    ]},
    { "op": "heap_print", "address": 6 },
    { "op": "print", "value": 777 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "?"
  }
}
```

> 1 回目の alloc/free でヒープが 1 ブロック分成長。以降はフリーリストから再利用されるため、99 回繰り返し後も ALLOC_HEAP_TOP は同じ。期待値は実装時に計算。

ヒープレイアウト計算:
- effective_global_size = 0 + 1 + 1 = 2
- FSBA テーブル開始: 8 + 2 = 10
- ALLOC_HEAP_TOP 初期値: 10 + 5 = 15
- alloc(3): total = 4 → class 1 (block 4), ALLOC_HEAP_TOP = 15 + 4 = 19

期待 stdout: `"19\n19\n777\n"`

##### 9. `reuse_general_ptr_001` — 汎用アロケータのポインタ再利用

32 セル超のブロックで First-Fit 再利用を検証。

```json
{
  "description": "汎用アロケータ: alloc(40)→free→alloc(40) で同じポインタが返される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 40 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 40 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n"
  }
}
```

##### 10. `reuse_general_split_reuse_001` — First-Fit 分割後の再利用

大きなブロックを free した後、小さなブロックを確保。残りのフリー領域からさらに確保できることを検証。

```json
{
  "description": "汎用アロケータ: 大ブロック free→小ブロック alloc (ブロック分割)→残りから再確保",
  "config": { "allocator": "fsba" },
  "vars": ["p_large", "p_small1", "p_small2"],
  "steps": [
    { "op": "alloc", "var": "p_large", "size": 80 },
    { "op": "heap_print", "address": 6 },
    { "op": "free", "var": "p_large" },
    { "op": "alloc", "var": "p_small1", "size": 33 },
    { "op": "alloc", "var": "p_small2", "size": 33 },
    { "op": "heap_print", "address": 6 },
    { "op": "assert_var_ne", "var1": "p_small1", "var2": "p_small2" },
    { "op": "print", "value": 666 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "?"
  }
}
```

> 81 セルのブロック(header含む)を free 後、34 セルのブロックを 2 回確保。1 回目は First-Fit でフリーブロックから分割。2 回目は分割残りから取得、またはバンプ拡張。ALLOC_HEAP_TOP が 1 回目と同じであれば、フリーブロックから 2 つとも確保されたことになる。

ヒープレイアウト計算:
- effective_global_size = 0 + 1 + 3 = 4
- FSBA テーブル開始: 8 + 4 = 12
- ALLOC_HEAP_TOP 初期値: 12 + 5 = 17
- alloc(80): total = 81 > 32 → 汎用, ALLOC_HEAP_TOP = 17 + 81 = 98
- free(p_large): 汎用フリーリストにプッシュ, ALLOC_HEAP_TOP = 98
- alloc(33): total = 34 > 32 → 汎用, First-Fit で 81 セルブロックから分割 → 34 使用 + 47 残り, ALLOC_HEAP_TOP = 98
- alloc(33): total = 34, First-Fit で残り 47 セルから分割 → 34 使用 + 13 残り, ALLOC_HEAP_TOP = 98

期待 stdout: `"98\n98\n666\n"`

##### 11. `reuse_mixed_class_independent_001` — クラス間の再利用独立性

異なるサイズクラスの free が互いに干渉せず、各クラスで正しく再利用されることを検証。

```json
{
  "description": "FSBA: 異なるクラスの free/alloc が互いに干渉せず正しく再利用",
  "config": { "allocator": "fsba" },
  "vars": ["small", "medium", "small2", "medium2"],
  "steps": [
    { "op": "alloc", "var": "small", "size": 1 },
    { "op": "alloc", "var": "medium", "size": 7 },
    { "op": "free", "var": "small" },
    { "op": "free", "var": "medium" },
    { "op": "alloc", "var": "small2", "size": 1 },
    { "op": "alloc", "var": "medium2", "size": 7 },
    { "op": "assert_var_eq", "var1": "small", "var2": "small2" },
    { "op": "assert_var_eq", "var1": "medium", "var2": "medium2" },
    { "op": "print", "value": 555 }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "555\n"
  }
}
```

> small は class 0, medium は class 2。free 後に同じサイズで再 alloc すると、各クラスのフリーリストから同じアドレスが返される。

##### 12. `reuse_roundup_same_class_001` — 切り上げでも同一クラスとして再利用

異なるリクエストサイズでも同一サイズクラスに切り上げられる場合、free→alloc で再利用されることを検証。

```json
{
  "description": "FSBA: alloc(2)→free→alloc(3) は同じ class 1 なので再利用される",
  "config": { "allocator": "fsba" },
  "vars": ["p1", "p2"],
  "steps": [
    { "op": "alloc", "var": "p1", "size": 2 },
    { "op": "free", "var": "p1" },
    { "op": "alloc", "var": "p2", "size": 3 },
    { "op": "assert_var_eq", "var1": "p1", "var2": "p2" },
    { "op": "load_print", "var": "p2" }
  ],
  "check": {
    "type": "alloc_io",
    "stdout": "3\n4\n5\n"
  }
}
```

> alloc(2): total=3 → class 1 (4 セル)。alloc(3): total=4 → class 1 (4 セル)。同じクラスなのでフリーリストから再利用。

### L3: E2E nospace テスト

#### 13. `alloc_reuse_same_ptr_001` — E2E ポインタ再利用

```nospace
# __alloc→__free→__alloc で同じポインタが返されることを検証 #
func: main() {
    let: p1(__alloc(2));
    __free(p1);
    let: p2(__alloc(2));
    # FSBA 同一クラスなので p1 == p2 のはず #
    __puti(p1 == p2);
    __putc(10);
    return: 0;
}
```

期待出力: `"1\n"` (p1 == p2 → 1)

#### 14. `alloc_reuse_func_frame_001` — 関数フレーム再利用

```nospace
# 同じ関数を 2 回呼び出し、フレームが再利用されることを検証 #
# 関数内でフレームポインタを取得する直接手段はないが、
# __alloc が返すアドレスで間接的に検証できる #
func: probe() {
    let: p(__alloc(1));
    *p = 42;
    __free(p);
    return: p;
}

func: main() {
    let: addr1(probe());
    let: addr2(probe());
    __puti(addr1 == addr2);
    __putc(10);
    return: 0;
}
```

期待出力: `"1\n"` (同じアドレスが再利用)

## テスト一覧まとめ

| # | テスト名 | 層 | カテゴリ | 検証内容 |
|---|---|---|---|---|
| 1 | `reuse_ptr_class0_001` | L1 | reuse | class 0 (2 セル) ポインタ再利用 |
| 2 | `reuse_ptr_class1_001` | L1 | reuse | class 1 (4 セル) ポインタ再利用 |
| 3 | `reuse_ptr_class2_001` | L1 | reuse | class 2 (8 セル) ポインタ再利用 |
| 4 | `reuse_ptr_class3_001` | L1 | reuse | class 3 (16 セル) ポインタ再利用 |
| 5 | `reuse_ptr_class4_001` | L1 | reuse | class 4 (32 セル) ポインタ再利用 |
| 6 | `reuse_heap_stable_001` | L1 | reuse | ALLOC_HEAP_TOP 非成長検証 |
| 7 | `reuse_lifo_order_001` | L1 | reuse | FSBA フリーリスト LIFO 順序 |
| 8 | `reuse_loop_stable_001` | L1 | reuse | 100 回ループでヒープ安定 |
| 9 | `reuse_general_ptr_001` | L1 | reuse | 汎用アロケータ (>32 セル) ポインタ再利用 |
| 10 | `reuse_general_split_reuse_001` | L1 | reuse | First-Fit 分割後の再利用 |
| 11 | `reuse_mixed_class_independent_001` | L1 | reuse | クラス間の再利用独立性 |
| 12 | `reuse_roundup_same_class_001` | L1 | reuse | サイズ切り上げでの同一クラス再利用 |
| 13 | `alloc_reuse_same_ptr_001` | L3 | E2E | nospace E2E ポインタ再利用 |
| 14 | `alloc_reuse_func_frame_001` | L3 | E2E | 関数フレーム再利用 |

## 実装計画

### Phase 1: インフラ整備 (小)

1. `tests/alloc_test.rs` に `AssertVarEq` 操作を追加
   - `AllocStep` enum にバリアント追加
   - `compile_assert_var_eq` メソッド実装
   - ラベル動的割当て（`alloc_label()` を使用）

### Phase 2: L1 分離テスト追加 (中)

1. `resources/tests_alloc/reuse/` ディレクトリ作成
2. 12 件のテスト JSON ファイル作成
3. `resources/tests_alloc/test-manifest.yaml` にエントリ追加
4. `cargo test --test alloc_test` で全テストパス確認

### Phase 3: L3 E2E テスト追加 (小)

1. nospace テストファイル作成 (`resources/tests/passes/builtins/`)
2. check.json 作成
3. `resources/tests/test-manifest.yaml` にエントリ追加
4. `cargo test` で全テストパス確認

## 備考

- ALLOC_HEAP_TOP のアドレスは `heap_layout::ALLOC_HEAP_TOP = 6` で固定
- FSBA_CLASS_COUNT = 5（クラス 0-4: ブロックサイズ 2, 4, 8, 16, 32）
- ヒープレイアウト計算における `effective_global_size` = `global_heap_size` + 1(counter) + `var_count`
- テスト JSON 内の `stdout` 期待値は、ヒープレイアウトとカウンタの動作から決定的に計算可能
- `reuse_heap_stable_001` と `reuse_loop_stable_001` の `stdout` 期待値は実装時にヒープレイアウトから計算して確定する（本ドキュメント内に計算例あり）
