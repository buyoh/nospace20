# 未初期化変数テスト失敗 - 原因分類と修正方針

## 概要

Phase 3 (strict-heap)、Phase 6 (randomize-uninit/randomize-heap) のテストで計6テストが失敗する。
各テストの失敗原因を「仕様変更によるもの」と「コンパイラのバグ」に分類し、修正方針を定める。

### 結論

**全6テストとも仕様変更（spec.md §4「変数の初期値は未定義」）が原因**であり、コンパイラのバグではない。

コンパイラの `generate_local_allocate` がヒープ領域をゼロクリアしない動作は、
新仕様「初期値を指定しない場合、読み出し時の値は不定となる」と整合しており、正常である。

## 失敗テスト一覧

| テスト名 | パス | strict-heap | interpreter-randomize | ws-self-randomize | 原因 |
|---------|------|:-----------:|:---------------------:|:-----------------:|:----:|
| `test_ok_coding_c001` | `c001` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` | 仕様変更 |
| `test_ok_coding_c002` | `c002` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` | 仕様変更 |
| `test_variables_var_basic_001` | `variables/var_basic_001` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` | 仕様変更 |
| `test_variables_var_init_hoisting` | `variables/var_init_hoisting` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` | 仕様変更 |
| `test_scope_scope_static_persist_001` | `scope/disabled_scope_static_persist_001` | `UninitializedHeap(8)` | trace mismatch | `UninitializedHeap(8)` | 仕様変更 |
| `test_example_queue` | `examples/e1-01-queue` | `UninitializedHeap(14)` | - | `UninitializedHeap(14)` | 仕様変更 |

※ `whitespace-self-strict` は `exclude_targets` で除外中（後述の通りコンパイラゼロクリア対応後に解除可能）

## 各テストの詳細分析

### 1. `c001.ns` — 仕様変更

```nospace
let:x;
__assert_not(x);   # ← x==0 を前提（旧仕様: 初期値0）
x=x+3;             # ← x=0+3=3 を前提
```

- `let:x;` で宣言後、初期化せずに `x` を読み出している
- 旧仕様では x=0 が保証されていたが、新仕様では未定義
- `y`, `z` は宣言直後に代入されるため問題なし

**修正**: `let:x;` → `let:x(0);`

### 2. `c002.ns` — 仕様変更

```nospace
let:i;
while:i-5+__trace(1){  # ← i==0 を前提（ループ回数が変わる）
    i=i+1;
```

- `let:i;` 後に初期化せず `while:i-5` で使用
- i=0 を前提にループ回数が決定される設計
- `let:j;` は直後に `j=5;` で代入されるため問題なし

**修正**: `let:i;` → `let:i(0);`

### 3. `var_basic_001.ns` — 仕様変更

```nospace
let:x;
__assert_not(x);  # 初期値は0 ← コメントが旧仕様を示す
x = x + 3;       # ← x=0+3=3 を前提
```

- 旧仕様の「初期値は0」を明示的にテストしている
- テストの目的自体が旧仕様の検証

**修正**: `let:x;` → `let:x(0);`、コメント「初期値は0」→「明示的に0で初期化」

### 4. `var_init_hoisting.ns` — 仕様変更

```nospace
__assert_not(a);  # ← ホイスティングされた a==0 を前提
a = 5;
let: a(3);
```

- ホイスティングにより `a` は `let:` 前にアクセス可能
- 旧仕様ではホイスティングされた変数の初期値は 0
- 新仕様では未定義 → `__assert_not(a)` は未定義動作

**修正**: `__assert_not(a);` を削除。ホイスティング自体のテスト（代入・初期化式のテスト）は維持。
具体的には `a = 5;` から開始し、ホイスティングによるアクセスと `let: a(3)` の初期化を検証する。

### 5. `disabled_scope_static_persist_001.ns` — 仕様変更

```nospace
func: counter() {
  static: count;       # ← 初期値未指定
  count = count + 1;   # ← count==0 を前提
  return: count;
}
```

- `static: count;` で初期化子なし
- `count + 1` が初回呼び出しで `0+1=1` を前提
- spec.md §9: 「static 変数は、グローバルスコープの変数と同じタイミングで初期化される」
  - 変数初期値は§4の規則に従い未定義

**修正**: `static: count;` の次行に `count = 0;` を追加、または初期化式が使えるなら `static: count(0);`

### 6. `e1-01-queue.ns` — 仕様変更

```nospace
let: idx_head;   # ← グローバル変数、初期値未指定
let: idx_tail;   # ← グローバル変数、初期値未指定
```

- `idx_head`, `idx_tail` がキューのインデックスとして 0 から開始する前提
- `UninitializedHeap(14)` = `GLOBAL_PTR(8) + offset(6)` = `idx_head` のアドレス
- データ配列 `data[5]` 自体は要素が初期化されずとも push 前に read しないため直接は問題ない

**修正**: `let: idx_head;` → `let: idx_head(0);`、`let: idx_tail;` → `let: idx_tail(0);`

## コンパイラ側の状況（バグではない）

### `generate_local_allocate` の動作

```
// 現在のコード (src/compiler_ws/builtin.rs)
heap_ptr += local_heap_size  // ポインタを進めるのみ、ゼロクリアなし
```

この動作は新仕様と整合している:
- spec.md §4: 「変数の初期値は未定義である」
- 初期化子付き宣言 `let: x(expr);` は `static_init_statements` / `root_statements` として
  代入文が生成されるため、正しく初期化される
- 初期化子なし `let: x;` は代入文が生成されず、ヒープ上の値は不定 → 仕様通り

### `generate_header` のグローバル領域

グローバル変数領域 (`GLOBAL_PTR` 〜 `GLOBAL_PTR + global_size`) もゼロクリアされない。
初期化子付きグローバル変数は `root_statements` で Store が生成されるため初期化される。
初期化子なしグローバル変数はヒープ値が不定 → 仕様通り。

### 将来的なゼロクリア追加の検討

仕様上は不要だが、安全性のためゼロクリアを追加することは可能。
ただし、仕様変更の検出（strict/randomize モードの目的）と矛盾するため、
現時点ではゼロクリアを追加しない方針とする。

## 修正方針まとめ

全テストの修正はテストコードの変更のみで対応する。コンパイラ・インタプリタの修正は不要。

| テスト | 変更内容 | 影響範囲 |
|--------|---------|---------|
| `c001.ns` | `let:x;` → `let:x(0);` | 最小限 |
| `c002.ns` | `let:i;` → `let:i(0);` | 最小限 |
| `var_basic_001.ns` | `let:x;` → `let:x(0);`、コメント更新 | テスト意図の変更 |
| `var_init_hoisting.ns` | `__assert_not(a);` 削除、ホイスティング+代入のテストを維持 | テスト設計の変更 |
| `disabled_scope_static_persist_001.ns` | `static: count;` → 初期化追加 | 最小限 |
| `e1-01-queue.ns` | `let: idx_head;` → `let: idx_head(0);` 等 | 最小限 |

修正後、以下の `exclude_targets` を解除:
- 全6テストの `whitespace-self-strict` を除外リストから削除

### 作業順序

1. テストコード修正（6ファイル）
2. 全テストターゲット（interpreter, whitespace-self, whitespace-self-strict, interpreter-randomize, whitespace-self-randomize）で通ることを確認
3. `exclude_targets: [whitespace-self-strict]` を削除
4. 本ドキュメントを done-task へ移動

## 更新履歴

- 2026-02-18: 初版作成（Phase 3 実装時に発見、strict-heap のみ）
- 2026-02-18: Phase 5 仕様変更・Phase 6 randomize 実装後に更新。`exclude_targets` から除外設定を削除し TODO として管理
- 2026-02-18: 全テストの原因を「仕様変更」と分類。修正方針を策定
