# 配列アクセスバグ修正・境界チェック削除

## 概要

配列実装 Phase 3 のインタプリタで 2 つの問題が発見された:

1. **変数インデックス解決のバグ**: ScopeResolver 用の `variable_indices_temp` が変数通し番号を使っており、配列サイズを考慮したスロットインデックスになっていない
2. **仕様変更**: 境界チェックを削除する。`p[i]` は `*(&p + i)` と同義であり、C言語と同様に境界チェックは行わない

## 問題1: 変数インデックスのバグ

### 再現コード

```nospace
func:main(){
let:arr[3];
let:i(1);
arr[1]=10;
x=arr[i];  // runtime error: array index out of bounds: index 10 but size 3
}
```

### 原因

`src/semantic_analyzer/mod.rs` の `syntactic_analyze_scope` 関数（620行目付近）:

```rust
let mut variable_indices_temp = BTreeMap::new();
for (idx, var) in scope.variables.iter().enumerate() {
    variable_indices_temp.insert(var.identifier.clone(), idx);  // ← バグ: idx は変数通し番号
}
```

一方、`ScopeBuilder::build()` メソッド（506行目付近）では:

```rust
let mut variable_indices = BTreeMap::new();
let mut slot_index = 0;
for (var_idx, var) in self.variables.iter().enumerate() {
    variable_indices.insert(var.identifier.clone(), slot_index);
    slot_index += var.array_size.unwrap_or(1);  // ← 正しい: 配列サイズを考慮
}
```

**結果**: ScopeResolver が使う `variable_indices_temp` では:

| 変数名 | 通し番号 (idx) | 正しいスロット |
|--------|----------------|----------------|
| arr    | 0              | 0              |
| i      | 1              | 3              |
| x      | 2              | 4              |

`resolve_variable("i")` が `local_index=1` を返すため、`scope[1]` (= `arr[1]` = 10) にアクセスしてしまう。

### 修正方針

`variable_indices_temp` の構築で、配列サイズを考慮したスロットインデックスを使う。`ScopeBuilder::build()` と同じロジックを適用する。

### 変更箇所

**`src/semantic_analyzer/mod.rs`** の `syntactic_analyze_scope` 関数内:

```rust
// 変更前
let mut variable_indices_temp = BTreeMap::new();
let mut variable_name_to_var_index_temp = BTreeMap::new();
for (idx, var) in scope.variables.iter().enumerate() {
    variable_indices_temp.insert(var.identifier.clone(), idx);
    variable_name_to_var_index_temp.insert(var.identifier.clone(), idx);
}

// 変更後
let mut variable_indices_temp = BTreeMap::new();
let mut variable_name_to_var_index_temp = BTreeMap::new();
let mut slot_index = 0;
for (idx, var) in scope.variables.iter().enumerate() {
    variable_indices_temp.insert(var.identifier.clone(), slot_index);
    variable_name_to_var_index_temp.insert(var.identifier.clone(), idx);
    slot_index += var.array_size.unwrap_or(1);
}
```

また、`temporary_scope` の `variable_count` も修正:

```rust
// 変更前
variable_count: scope.variables.len(),

// 変更後
variable_count: slot_index,
```

## 問題2: 境界チェックの削除

### 仕様

`p[i]` は `*(&p + i)` と同義。C言語と同様に実行時の境界チェックは行わない。

### 変更箇所

**`src/interpreter/exec.rs`** の3箇所から境界チェックを削除:

1. **ArrayAccess の読み取り** (`interpret_expression` 内):
   ```rust
   // 削除: 境界チェック
   // if index < 0 || index >= *array_size as i64 { ... }
   ```

2. **ArrayAccess への代入** (`interpret_operation2` の `Assign` 内):
   ```rust
   // 削除: 境界チェック
   // if index < 0 || index >= *array_size as i64 { ... }
   ```

3. **&arr[i] の参照取得** (`interpret_operation1` の `Ref` 内):
   ```rust
   // 削除: 境界チェック
   // if index < 0 || index >= *array_size as i64 { ... }
   ```

### `ExecExpression::ArrayAccess` の `array_size` フィールドについて

境界チェックを削除するため `array_size` フィールドは不要になるが、以下の理由から今回のスコープ内では保持する:

- Phase 4 (compiler_ws) で利用する可能性がある
- 将来的にオプショナルな境界チェックモードを追加する可能性がある
- 削除は影響範囲が広い（semantic_analyzer のテストも変更が必要）

### テストの変更

境界チェック用テスト（`resources/tests/fails/array-out-of-bounds.*`, `array-negative-index.*`）を削除し、`test-manifest.yaml` からもコメントアウト部分を削除する。

## テスト影響

### 修正により通るようになるテスト

- `test_array_basic`: while ループ内の配列アクセスが修正される
- `test_array_reference`: ポインタ演算による配列操作が修正される
- `test_array_static`: static 配列の操作が修正される

### 削除するテスト

- `array-out-of-bounds` (境界チェック関連)
- `array-negative-index` (境界チェック関連)
