# 残りの _ws_self テスト失敗 (5件) 調査・修正設計

## 概要

[fix-ws-self-label-duplication.md](fix-ws-self-label-duplication.md) の修正により、
15件の失敗テストのうち10件が成功に変わったが、残り5件が依然として失敗している。

調査の結果、これらは **ラベル重複ではなく、関数呼び出し規約（calling convention）のバグ** であることが判明した。

## 残りの失敗テスト (5件)

| # | テスト名 | 失敗パターン | 詳細 |
|---|---|---|---|
| 1 | test_example_fibonacci_ws_self | Suspended (無限ループ) | ステップ数上限超過 |
| 2 | test_example_qsort_ws_self | 出力不一致 | 期待値あり、実際は空出力 |
| 3 | test_legacy_014_ws_self | 出力不一致 | 期待: "1-12-2", 実際: "8-81-1" |
| 4 | test_legacy_015_ws_self | Suspended | ステップ数上限超過 |
| 5 | test_legacy_020_ws_self | 出力不一致 | 期待: "0111;1010;1010;0111;0101;", 実際: "0111;0111;1010;" |

---

## 根本原因の特定

### 発見した2つのバグ

#### Bug A: パラメータ格納順序の誤り (statement.rs)

`generate_function_definition` において、ローカルフレーム確保 (`generate_local_allocate`) が
引数のコピー **より前** に実行される。`generate_local_allocate` は `old_LHB` (旧 LOCAL_HEAP_BEGIN)
をデータスタック上に退避するため、**引数の上に old_LHB が積まれる**。

```
呼び出し側: push arg1; call func
関数エントリ時: stack = [..., arg1]
allocate 後:    stack = [..., arg1, old_LHB]  ← old_LHB が上に来る
```

その後のパラメータ格納コードは `swap; store` でスタック最上位要素を取得するが、
取得されるのは **old_LHB であり引数ではない**。結果として:

- パラメータの値 = old_LHB（間違い）
- 引数はスタックに残り、dealloc 時に old_LHB として扱われる（間違い）

**legacy_014 のスタックトレースによる実証:**

```
f(1) を呼び出し:
  stack at entry: [1]
  allocate 後:    [1, old_LHB=8]
  パラメータ格納: heap[8] = 8 (← old_LHB を格納、1 ではない!)
  → __puti(x) → 8 を出力  (期待: 1)
  → __puti(-x) → -8 を出力  (期待: -1)
  dealloc 時: heap[2] = 1 (← 引数 1 で LHB を上書き! 8 ではない!)
```

#### Bug B: return 文での deallocate 時のスタック不整合 (statement.rs)

`generate_return` は返り値を評価してからスタックに積み、その後 `generate_local_deallocate()` を
呼ぶ。しかし deallocate コードの `push 2; swap; store` は **スタック最上位が old_LHB** であること
を前提としており、返り値がある場合はズレる:

```
return: expr 実行時:
  stack: [old_LHB, return_value]
  deallocate の push 2; swap; store:
    → heap[2] = return_value (間違い！old_LHB ではない)
    → stack に old_LHB が残る (呼び出し元はこれを return 値と誤認)
```

### バグの相殺パターン

Bug A と B は特定の条件下で **相殺** し、正しい結果を返すように見える:

1. **Bug A**: パラメータに old_LHB が入り、引数がスタクに残る
2. **Bug B**: return 時に引数 (fake old_LHB) がからスタク上に残り返り値として使われる

**単純な `return: a` (引数をそのまま返す) 場合:**
- x = old_LHB (Bug A)
- return: x → return_value = old_LHB → deallocate が heap[2] = old_LHB (偶然正しい)
- caller は元の引数を受け取る (偶然正しい)

しかし、`return: a * 2` のように引数を変換すると、相殺が破れて不正な値になる。

### 通るテストが存在する理由

1. **debug 組み込み関数のみ使用**: `__assert`, `__trace` は WS モードで NOOP。
   `func_args_001`, `ref_swap_001` 等は stdout 出力がなく、テストが実質無効
2. **引数 == old_LHB の偶然の一致**: `puts(&g)` のように引数が `&(最初のローカル変数)`
   であり LHB と同値になるケース（`test_example_puts`）

---

## 修正設計

### Fix A: パラメータ格納を allocate の前に移動

**対象ファイル**: `src/compiler_ws/statement.rs` の `generate_function_definition`

**現在の順序** (間違い):
1. `generate_local_allocate` → old_LHB をスタックに退避
2. パラメータ格納 → `swap; store` で old_LHB を格納してしまう

**修正後の順序**:
1. パラメータ格納 → `LOCAL_HEAP_END` をベースアドレスとして使用
2. `generate_local_allocate` → old_LHB をスタックに退避

**ポイント**: allocate 前の `LOCAL_HEAP_END` は新しいフレームの `LOCAL_HEAP_BEGIN`
と同じ値になる。よって `heap[LOCAL_HEAP_END + offset]` に格納すれば、
allocate 後に `heap[LOCAL_HEAP_BEGIN + offset]` で正しく参照できる。

```rust
// 修正前
prog.append(builtin::generate_local_allocate(local_ctx.local_heap_size()));
for i in (0..func.arg_indices.len()).rev() {
    let offset = func.arg_indices.get(i).copied().unwrap_or(i) as i64;
    prog.extend([
        Push(WsNumber(offset)),
        Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)),  // ← allocate 後の LHB
        Retrieve, Add, Swap, Store,
    ]);
}

// 修正後
for i in (0..func.arg_indices.len()).rev() {
    let offset = func.arg_indices.get(i).copied().unwrap_or(i) as i64;
    prog.extend([
        Push(WsNumber(offset)),
        Push(WsNumber(heap_layout::LOCAL_HEAP_END)),    // ← allocate 前の LHE = 将来の LHB
        Retrieve, Add, Swap, Store,
    ]);
}
prog.append(builtin::generate_local_allocate(local_ctx.local_heap_size()));
```

**スタックトレース検証 (f(1) の場合):**
```
entry: stack = [1]
パラメータ格納: heap[heap[3]+0] = heap[8] = 1 ✓, stack = []
allocate: stack = [old_LHB=8]
x = heap[8] = 1 ✓
dealloc: heap[2] = old_LHB=8 ✓
```

### Fix B: return 文で swap を追加

**対象ファイル**: `src/compiler_ws/statement.rs` の `generate_return`

返り値を評価した後、deallocate の前に `Swap` を挿入する。これにより
old_LHB がスタック最上位に来て、deallocate が正しく動作する。

```rust
// 修正前
fn generate_return(ctx, expr) {
    prog.append(generate_expression(ctx, expr)?);  // stack: [old_LHB, return_value]
    prog.append(generate_local_deallocate());       // heap[2] = return_value ← BUG
    prog.push(Return);                              // returns old_LHB ← BUG
}

// 修正後
fn generate_return(ctx, expr) {
    prog.append(generate_expression(ctx, expr)?);  // stack: [old_LHB, return_value]
    prog.push(Swap);                               // stack: [return_value, old_LHB]
    prog.append(generate_local_deallocate());       // heap[2] = old_LHB ✓
    prog.push(Return);                              // returns return_value ✓
}
```

**注意**: デフォルト return (関数末尾) は deallocate → push 0 → ret の順で
既に正しいため修正不要。

### 既存テストへの影響

- **修正により全 5件の失敗テストが解決される見込み**
- 通っていたテストは:
  - debug noop 系: 引き続き通る (出力検証なし)
  - puts 系 (引数==old_LHB の偶然一致): 正しい動作に変わるため引き続き通る
  - default return のみ: 修正は前後とも正しいので変化なし

---

## ステータス

- [x] 失敗テストのリスト作成
- [x] 各テストのソースコード確認
- [x] コンパイル結果の詳細分析 (mnemonic + スタックトレース)
- [x] 根本原因の特定 (Bug A: パラメータ格納順序, Bug B: return deallocate 不整合)
- [x] 修正設計 (Fix A + Fix B)
- [x] Fix A 実装完了
- [x] Fix B 実装完了
- [ ] 全テスト通過確認（1件残存）

### Fix A 実装結果 (2026-02-17)

**実装内容:**
- `src/compiler_ws/statement.rs` の `generate_function_definition` でパラメータ格納を `generate_local_allocate` の前に移動
- パラメータ格納時のベースアドレスを `LOCAL_HEAP_BEGIN` から `LOCAL_HEAP_END` に変更

**テスト結果:**
- ws_self テスト: 112 passed; 2 failed (改善前: 109 passed; 5 failed)
- 解決したテスト (3件):
  - test_legacy_014_ws_self ✓
  - test_legacy_015_ws_self ✓
  - test_legacy_020_ws_self ✓
- 残りの失敗テスト (2件):
  - test_example_fibonacci_ws_self (出力: 期待 "1\n", 実際 "8\n")
  - test_example_qsort_ws_self

**考察:**
Fix A だけで 3/5 件のテストが解決。残りの 2 件は return 文で値を変換して返すケースで、
Bug B (return 時の swap 不足) の影響を受けていると推測される。Fix B の実装が必要。

### Fix B 実装結果 (2026-02-17)

**実装内容:**
- `src/compiler_ws/statement.rs` の `generate_return` で返り値評価後、`generate_local_deallocate` の前に `Swap` 命令を追加
- これにより、スタックが [return_value, old_LHB] となり、deallocate が正しく動作

**テスト結果:**
- ws_self テスト: 113 passed; 1 failed (Fix A のみ: 112 passed; 2 failed)
- 新たに解決したテスト (1件):
  - test_example_fibonacci_ws_self ✓
- 残りの失敗テスト (1件):
  - test_example_qsort_ws_self (出力: 期待値あり、実際は空出力)

**考察:**
Fix B により fibonacci テストが解決。qsort の失敗は出力が空であり、
元の調査で想定していた Bug A/B とは別の問題の可能性がある。
別途調査が必要。

## 関連ドキュメント

- [fix-ws-self-label-duplication.md](fix-ws-self-label-duplication.md) - 既に修正した10件のラベル重複バグ
- [whitespace-self-test-failures.md](whitespace-self-test-failures.md) - 元の15件の失敗調査
