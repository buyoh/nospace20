# 後置添字演算子 `(expr)[i]` の脱糖仕様変更

## 概要

後置添字演算子 `(expr)[i]` の脱糖ルールを変更し、`arr[i]` と一貫した動作にする。
同時に、既存テストの改善・追加を行い、添字演算子が想定した箇所に書き込み・読み取りを行っているかを検証可能にする。

## 背景

### 問題1: テスト不備

既存テスト (`postfix_subscript_read.ns`, `postfix_subscript_write.ns`, `postfix_subscript_compound.ns`) は `(*p)[i]` パターンを使用しており、演算子がエラーなくコンパイル・実行されることは確認できる。しかし、二重間接参照を経由するため、書き込み先が本当に想定した箇所であるかを明確に検証するテストとしては不十分。

### 問題2: 仕様不一致

**spec.md の記述:**

```
arr[i] は *(&arr + i) と同義
```

`arr[i]` では、arr は**実体**（変数自体）を指し、`&arr` でアドレスを取得してからオフセットする。

**現在の実装 (tree_parser/expression/mod.rs):**

- `arr[i]` → `Expression::ArrayAccess(id, index_expr)` — 意味解析で `*(&arr + i)` 相当に処理（arr は実体）
- `(expr)[i]` → `*(expr + i)` に脱糖 — expr は**参照（ポインタ）**を返す必要がある

つまり、`[]` の適用対象が識別子か式かで、実体を指すか参照を返すかが異なる。

**具体例:**

```
let: a;
let: arr[4];
# arr[i] — arr は実体。*(&arr + i) 相当
# (arr)[i] — 現在: *(arr + i) 。arr の値がポインタとして扱われてしまう
# (arr)[i] — 提案: *(&arr + i) 。arr[i] と同じ動作
```

## 仕様変更

### 変更内容

後置添字演算子 `(expr)[i]` の脱糖ルールを変更する:

| | 旧仕様 | 新仕様 |
|---|---|---|
| `arr[i]` (識別子) | `*(&arr + i)` — arr は実体 | 変更なし |
| `(expr)[i]` (式) | `*(expr + i)` — expr は参照を返す | `*(&(expr) + i)` — expr は実体を返す |

新仕様では `(expr)[i]` は `*(&(expr) + i)` に脱糖される。  
`&` は変数またはArrayAccessにのみ適用可能であるため、`(expr)` は lvalue（変数、配列要素、デリファレンス結果）である必要がある。

### 動作例（新仕様）

```
# 変数に対する添字（新旧同じ）
let: a;
let: b;
a[0]  # → *(&a + 0) = a
a[1]  # → *(&a + 1) = b (隣接変数)

# 式に対する添字（新仕様）
let: p;
p = &a;
(*p)[0]  # → *(&(*p) + 0) = *(p + 0) = *p = a  ← &と*が打ち消し合う
(*p)[1]  # → *(&(*p) + 1) = *(p + 1) = *((&a) + 1) = b
```

**注意:** 旧仕様の `(*p)[i] = *((*p) + i)` は `*p` の値をポインタとして扱い、そこからオフセットした。新仕様の `(*p)[i] = *(&(*p) + i) = *(p + i)` は `*p` の**アドレス**からオフセットする。意味が異なる。

## 実装計画

### Step 1: tree_parser の脱糖ルール変更

**対象ファイル:** `src/tree_parser/expression/mod.rs`

後置添字ループ（約 L240 付近）の脱糖コードを変更する。

**現在のコード:**

```rust
// (expr)[i] → *(expr + i) に脱糖
let plus_expr = self.located(
    Expression::Operation2(Operator2::Plus, result, index_expr),
    start, end,
);
result = self.located(
    Expression::Operation1(Operator1::Deref, plus_expr),
    start, end,
);
```

**変更後:**

```rust
// (expr)[i] → *(&(expr) + i) に脱糖
let ref_expr = self.located(
    Expression::Operation1(Operator1::Ref, result),
    start, end,
);
let plus_expr = self.located(
    Expression::Operation2(Operator2::Plus, ref_expr, index_expr),
    start, end,
);
result = self.located(
    Expression::Operation1(Operator1::Deref, plus_expr),
    start, end,
);
```

### Step 2: 既存テストの修正

既存テストは旧仕様の `(*p)[i] = *((*p) + i)` に基づいている。新仕様では `(*p)[i] = *(p + i)` となるため、テストのセットアップと期待値を修正する。

#### postfix_subscript_read.ns

旧テストでは `(*p)[i]` が `*(*p + i)` として動作していた（p→q→a へのダブルポインタ）。  
新テストでは `(*p)[i]` が `*(p + i)` として動作する（p→a への直接ポインタアクセス）。

テスト内容を、`(expr)[i]` 脱糖が正しく機能するシンプルなケースに書き換える:

```nospace
# 後置添字演算子による読み取り: (expr)[i] #
# (expr)[i] は *(&(expr) + i) に脱糖される #
func: __main() {
  let: a;
  let: b;
  let: p;
  a = 100;
  b = 200;
  p = &a;   # p = a のアドレス #
  # (*p)[0] → *(&(*p) + 0) = *(p + 0) = *p = a = 100 #
  __puti((*p)[0]);
  __putc(10);
  # (*p)[1] → *(&(*p) + 1) = *(p + 1) → メモリ上 p の隣 #
  # 変数 a, b, p の配置に依存するため、独立した確認が難しい #
  # 代わりに配列アクセスで確認 #
  let: arr[3];
  arr[0] = 10;
  arr[1] = 20;
  arr[2] = 30;
  let: q;
  q = &arr;
  # (*q)[0] = *(q + 0) = *q = *(&arr) = arr[0] = 10 #
  __puti((*q)[0]);
  __putc(10);
  # (*q)[1] = *(q + 1) = *(&arr + 1) = arr[1] = 20 #
  __puti((*q)[1]);
  __putc(10);
  # (*q)[2] = *(q + 2) = *(&arr + 2) = arr[2] = 30 #
  __puti((*q)[2]);
}
```

期待出力: `100\n10\n20\n30`

#### postfix_subscript_write.ns

添字演算子で書き込んだ値が、想定した変数に反映されていることを明確に検証する:

```nospace
# 後置添字演算子による書き込み: (expr)[i] = val #
# (expr)[i] は *(&(expr) + i) に脱糖される #
func: __main() {
  let: arr[3];
  arr[0] = 0;
  arr[1] = 0;
  arr[2] = 0;
  let: p;
  p = &arr;
  # (*p)[0] = 111 → *(&(*p) + 0) = *(p + 0) に 111 を格納 → arr[0] = 111 #
  (*p)[0] = 111;
  __puti(arr[0]);
  __putc(10);
  # (*p)[1] = 222 → *(p + 1) に 222 を格納 → arr[1] = 222 #
  (*p)[1] = 222;
  __puti(arr[1]);
  __putc(10);
  # (*p)[2] = 333 → *(p + 2) に 333 を格納 → arr[2] = 333 #
  (*p)[2] = 333;
  __puti(arr[2]);
}
```

期待出力: `111\n222\n333`

**ポイント:** `arr[i]` で書き込み先を確認しているため、`(expr)[i]` の書き込みが想定したメモリ位置に正しく行われていることを検証できる。

#### postfix_subscript_compound.ns

同様に配列を使った複合代入テストに変更:

```nospace
# 後置添字演算子による複合代入: (expr)[i] += val #
# (expr)[i] は *(&(expr) + i) に脱糖される #
func: __main() {
  let: arr[3];
  arr[0] = 100;
  arr[1] = 200;
  arr[2] = 300;
  let: p;
  p = &arr;
  # (*p)[0] += 11 → arr[0] += 11 = 111 #
  (*p)[0] += 11;
  __puti(arr[0]);
  __putc(10);
  # (*p)[1] += 22 → arr[1] += 22 = 222 #
  (*p)[1] += 22;
  __puti(arr[1]);
  __putc(10);
  # (*p)[2] += 33 → arr[2] += 33 = 333 #
  (*p)[2] += 33;
  __puti(arr[2]);
}
```

期待出力: `111\n222\n333`

### Step 3: check.json の更新

各テストの `.check.json` を新しい期待出力に合わせて更新する。

### Step 4: テストの実行と検証

```bash
cargo test --test code_test
```

全テストが通ることを確認する。なお、他の `arr[i]` 形式を使用するテスト（`Expression::ArrayAccess`）は影響を受けないはず。

### Step 5: spec.md の更新（検討）

spec.md に `(expr)[i]` の脱糖ルールに関する記述を追加するかは、仕様の反映タスクとして別途検討。

## 進捗

- [x] Step 1: `src/tree_parser/expression/mod.rs` の脱糖ルール変更（`*(expr + i)` → `*(&(expr) + i)`）
- [x] Step 2: `src/semantic_analyzer/expression.rs` に `&(*x) = x` の恒等式を追加（Deref 結果への Ref 適用を許容）
- [x] Step 3: 既存テスト3件（`postfix_subscript_read.ns`, `postfix_subscript_write.ns`, `postfix_subscript_compound.ns`）を新仕様に書き換え
- [x] Step 4: `.check.json` の期待出力を更新
- [x] Step 5: largeテスト `cargo test --test code_test postfix_subscript` → 21テスト全通過

### 未解決

- `src/tree_parser/expression/test.rs` の3ユニットテストが旧仕様の AST 構造を期待しているため失敗
  - `test_parse_postfix_subscript_deref_paren`
  - `test_parse_postfix_subscript_deref_paren_index_1`  
  - `test_parse_postfix_subscript_expr_paren`
  - 調査ドキュメント: `ai-docs/task/fix-tree-parser-postfix-subscript-unit-tests.md`


### 変更対象ファイル

| ファイル | 変更内容 |
|---|---|
| `src/tree_parser/expression/mod.rs` | 後置添字ループの脱糖ルール変更（1箇所） |
| `resources/tests/passes/postfix_subscript_read.ns` | テスト内容の書き換え |
| `resources/tests/passes/postfix_subscript_write.ns` | テスト内容の書き換え |
| `resources/tests/passes/postfix_subscript_compound.ns` | テスト内容の書き換え |
| `resources/tests/passes/postfix_subscript_read.check.json` | 期待出力の更新 |
| `resources/tests/passes/postfix_subscript_write.check.json` | 期待出力の更新 |
| `resources/tests/passes/postfix_subscript_compound.check.json` | 期待出力の更新 |

### 影響を受けないもの

- `Expression::ArrayAccess` のパス（`arr[i]` 形式）: tree_parser の Identifier 分岐で処理されるため、後置ループの変更の影響を受けない
- 意味解析 (`semantic_analyzer`): `&` + `+` + `*` の組み合わせは既存のロジックで処理可能
- コンパイラ (`compiler_ws`): `Ref`, `Plus`, `Deref` の個別命令は既に実装済み
- `Expression::ArrayAccess` を使う全テスト: 影響なし

### リスク

- `&(expr)` が適用可能な expr の種類が制限される（Variable, ArrayAccess, Deref のみ）。任意の式に対して `[i]` を使おうとするとコンパイルエラーになる。これは意図した動作。
- 旧仕様に依存するユーザーコードがあれば壊れるが、nospace は開発中の言語であり互換性の問題は軽微。
