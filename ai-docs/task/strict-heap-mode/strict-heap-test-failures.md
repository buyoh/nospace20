# strict-heap テスト失敗調査

## 概要

Phase 3 で `whitespace-self-strict` テストを追加した結果、6件のテストが `UninitializedHeap` エラーで失敗した。
これらは `exclude_targets: [whitespace-self-strict]` で除外済み。

## 失敗したテスト

| テスト名 | パス | エラー |
|---------|------|--------|
| `test_ok_coding_c001_ws_self_strict` | `c001` | `UninitializedHeap(8)` |
| `test_ok_coding_c002_ws_self_strict` | `c002` | `UninitializedHeap(8)` |
| `test_scope_scope_static_persist_001_ws_self_strict` | `scope/disabled_scope_static_persist_001` | `UninitializedHeap(8)` |
| `test_variables_var_basic_001_ws_self_strict` | `variables/var_basic_001` | `UninitializedHeap(8)` |
| `test_variables_var_init_hoisting_ws_self_strict` | `variables/var_init_hoisting` | `UninitializedHeap(8)` |
| `test_example_queue_ws_self_strict` | `examples/e1-01-queue` | `UninitializedHeap(14)` |

## 原因分析

### アドレス 8 への UninitializedHeap

アドレス 8 は nospace コンパイラの内部アドレス体系で「ローカル変数領域の先頭付近」にあたると推定される。
アドレス 2〜6 あたりは予約アドレス:
- 0: global heap pointer
- 1: local heap pointer
- 2: LOCAL_HEAP_BEGIN
- 3: LOCAL_HEAP_END
- 4〜: 使用されるアドレス

`var_basic_001` や `var_init_hoisting` では変数宣言時にヒープ領域を確保するが、
`generate_local_allocate` はヒープポインタを進めるだけでゼロクリアを行わないため、
確保した領域への最初の `retrieve`（読み出し）が未初期化となる。

例: `var x;` → ヒープアドレス確保 → `retrieve x` → UninitializedHeap

### アドレス 14 への UninitializedHeap（queue テスト）

`e1-01-queue` は配列やキューを使う複雑なテスト。
アドレス 14 も同様に未初期化変数の読み出しによるものと推定。

## 対処方針

これらの失敗は「コンパイラが変数確保後に初期値 0 で初期化するコードを生成していない」ことに起因する。
修正方法:
1. `generate_local_allocate` でゼロクリアコードを生成する（別タスク）
2. または仕様として「変数の初期値は未定義」とし、ユーザが明示的に初期化することを要求する（`undefined-variable-init.md` 参照）

現状は上記が別タスク（Phase 5/6）で対応予定のため、これらのテストを除外する。

## 更新履歴

- 2026-02-18: 初版作成（Phase 3 実装時に発見）
