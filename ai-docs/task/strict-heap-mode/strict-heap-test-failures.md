# 未初期化変数アクセス - テスト失敗 TODO

## 概要

Phase 3 (strict-heap) および Phase 6 (randomize-uninit) のテストで、同じ原因により複数のテストが失敗する。
Phase 5 の仕様変更（変数初期値を「未定義」に変更）に伴い、これらの失敗は**仕様上の期待動作**を示す可能性がある。
`exclude_targets` による除外は行わず、コンパイラ/インタプリタの修正 TODO として管理する。

## 失敗するテストと失敗モード

| テスト名 | パス | strict-heap | interpreter-randomize | ws-self-randomize |
|---------|------|:-----------:|:---------------------:|:-----------------:|
| `test_ok_coding_c001` | `c001` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` |
| `test_ok_coding_c002` | `c002` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` |
| `test_scope_scope_static_persist_001` | `scope/disabled_scope_static_persist_001` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` |
| `test_variables_var_basic_001` | `variables/var_basic_001` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` |
| `test_variables_var_init_hoisting` | `variables/var_init_hoisting` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` |
| `test_example_queue` | `examples/e1-01-queue` | `UninitializedHeap(14)` | - | `UninitializedHeap(14)` |

※ `whitespace-self-strict` のみ `exclude_targets` で除外中（コンパイラバグが確定しているため）

## 根本原因

### Whitespace コンパイラ: 変数ゼロクリア未実装

`generate_local_allocate` はヒープポインタを進めるだけで、確保した変数領域のゼロクリアを行わない。
このため、strict-heap モード・randomize-heap モードでは未初期化ヒープアクセスとして検出される。

```
var x;  →  ヒープアドレス確保（ポインタ +1）
x を読む →  retrieve → UninitializedHeap / ランダム値
```

### nospace インタプリタ: 初期値 0 への暗黙依存

`c001`, `c002`, `var_basic_001`, `var_init_hoisting`, `scope_static_persist_001` は、
`var x;` 宣言後に x を明示的に初期化せずに読み出している（初期値 0 を前提としたコード）。
randomize-uninit モードではランダム値が入るため trace の結果が変わり、テストが失敗する。

## 修正 TODO

### TODO-1: Whitespace コンパイラの変数ゼロクリア

`generate_local_allocate` で変数領域を確保した後、Store 命令でゼロクリアするコードを生成する。

```
# 現在
heap_ptr += 1  # アドレス確保のみ

# 修正後
heap_ptr += 1
heap[heap_ptr - 1] = 0  # ゼロクリア
```

これにより strict-heap・randomize テストが通るようになる。
対象ファイル: `src/compiler_ws/` (generate_local_allocate 関連)

### TODO-2: テストコードの修正（仕様変更への追従）

`var x;` で初期値 0 を暗黙的に前提しているテストは、仕様変更に従い `let: x(0);` のように
明示的な初期化を追加するか、テストの期待値を更新する。

対象テスト:
- `resources/tests/passes/c001.ns`
- `resources/tests/passes/c002.ns`
- `resources/tests/passes/variables/var_basic_001.ns`
- `resources/tests/passes/variables/var_init_hoisting.ns`
- `resources/tests/passes/scope/disabled_scope_static_persist_001.ns`
- `resources/tests/passes/examples/e1-01-queue.ns`

## 更新履歴

- 2026-02-18: 初版作成（Phase 3 実装時に発見、strict-heap のみ）
- 2026-02-18: Phase 5 仕様変更・Phase 6 randomize 実装後に更新。`exclude_targets` から除外設定を削除し TODO として管理
