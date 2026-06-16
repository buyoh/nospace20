# 非配列変数への `[]` 演算子適用の設計

## 概要

仕様追記: `arr[i]` は `*(&arr + i)` と同義。

現状、非配列変数に `[]` 演算子を使うと意味解析で `"'x' is not an array"` エラーが発生する。
仕様により、`[]` は任意の変数に対して使用可能であるべき。非配列変数 `x` に対する `x[i]` は `*(&x + i)` と等価になる。

## 現状分析

### エラー発生箇所

`src/semantic_analyzer/mod.rs` の2箇所で `"is not an array"` エラーが発生:

1. **L220-L231**: 通常の `Expression::ArrayAccess` 処理
   - `get_array_size()` が `Some(None)` を返すと（通常変数の場合）エラー
2. **L48-L61**: `&arr[i]` での `Operator1::Ref` + `ArrayAccess` 処理  
   - 同様に `get_array_size()` チェックでエラー

### 影響範囲

| モジュール | 変更の必要性 | 理由 |
|---|---|---|
| tree_parser | **なし** | `x[i]` は既に `ArrayAccess("x", expr)` として正しくパースされる |
| semantic_analyzer | **あり** | `get_array_size` による配列チェックを緩和する必要がある |
| interpreter | **なし** | `_array_size` は使用されておらず、境界チェックもない。既に `*(&arr + i)` と同義の処理 |
| compiler_ws | **なし** | 同上。`_array_size` は未使用 |

### `ExecExpression::ArrayAccess` の `array_size` フィールド

```rust
ArrayAccess(IdentifierRef, Box<ExecExpression>, usize)
//                                               ^^^^^ array_size
```

- interpreter: 3箇所すべてで `_array_size` として無視
- compiler_ws: 4箇所すべてで `_` として無視

→ `array_size` は現在どこでも使用されていない。非配列変数の場合は `1` を設定すればよい。

## 設計

### semantic_analyzer の変更

`src/semantic_analyzer/mod.rs` の2箇所を修正:

#### 変更1: 通常の `ArrayAccess` (L220-L234)

**変更前:**
```rust
Expression::ArrayAccess(name, index_expr) => {
    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
        vec![code_parse_error!(format!("undefined variable: {}", name))]
    })?;
    let array_size = parent_resolver
        .get_array_size(name)
        .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?
        .ok_or_else(|| vec![code_parse_error!(format!("'{}' is not an array", name))])?;
    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;
    Ok(Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)))
}
```

**変更後:**
```rust
Expression::ArrayAccess(name, index_expr) => {
    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
        vec![code_parse_error!(format!("undefined variable: {}", name))]
    })?;
    // arr[i] は *(&arr + i) と同義。配列でなくてもインデックスアクセス可能。
    let array_size = parent_resolver
        .get_array_size(name)
        .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?
        .unwrap_or(1);
    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;
    Ok(Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)))
}
```

#### 変更2: `&arr[i]` のケース (L48-L61)

**変更前:**
```rust
Expression::ArrayAccess(name, index_expr) => {
    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
        vec![code_parse_error!(format!("undefined variable: {}", name))]
    })?;
    let array_size = parent_resolver
        .get_array_size(name)
        .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?
        .ok_or_else(|| vec![code_parse_error!(format!("'{}' is not an array", name))])?;
    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;
    Ok(Box::new(ExecExpression::Operation1(
        Operator1::Ref,
        Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)),
    )))
}
```

**変更後:**
```rust
Expression::ArrayAccess(name, index_expr) => {
    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
        vec![code_parse_error!(format!("undefined variable: {}", name))]
    })?;
    // arr[i] は *(&arr + i) と同義。配列でなくてもインデックスアクセス可能。
    let array_size = parent_resolver
        .get_array_size(name)
        .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?
        .unwrap_or(1);
    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;
    Ok(Box::new(ExecExpression::Operation1(
        Operator1::Ref,
        Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)),
    )))
}
```

### テストの変更

#### 1. 削除/変更が必要なテスト

- **`src/semantic_analyzer/tests.rs`** の `test_error_not_an_array`:
  非配列変数への `[]` アクセスが合法になるため、このテストを成功テストに変更する。

- **`resources/tests/fails/compile/not_an_array_001.ns`** / `.check.json`:
  非配列変数への `[]` アクセスはエラーでなくなるため、このテストケースを `passes/` に移動するか削除する。

#### 2. 追加が必要なテスト

`resources/tests/passes/` に以下のテストケースを追加:

- **`index_operator_non_array_001.ns`**: 非配列変数への `[]` 読み取り  
  参照を通じたインデックスアクセスが `*(&x + i)` と同等であることを確認
- **`index_operator_non_array_002.ns`**: 非配列変数への `[]` 書き込み  
  `x[0] = value` が `*(&x + 0) = value` と同等であることを確認
- **`index_operator_non_array_003.ns`**: `&x[i]` が参照演算と一致することの確認

## 作業順序

1. `src/semantic_analyzer/mod.rs` の2箇所を修正（`.ok_or_else` → `.unwrap_or(1)`）
2. `src/semantic_analyzer/tests.rs` の `test_error_not_an_array` を正常系テストに変更
3. `resources/tests/fails/compile/not_an_array_001` を削除
4. 新テストケースを `resources/tests/passes/` に追加
5. `cargo test` で全テスト通過を確認

## リスク・注意点

- **インタプリタ・コンパイラへの変更は不要**: `array_size` は全箇所で無視されており、ランタイム動作に影響なし
- **メモリ安全性**: `x[i]` で `i != 0` の場合、隣接変数のメモリにアクセスすることになる。これは仕様通りの動作（C言語のポインタ演算と同等、境界チェックなし）
- **既存テストへの影響**: `not_an_array_001` 以外の既存テストには影響なし

## 進捗

### 2026-02-17

- [x] `src/semantic_analyzer/mod.rs` の2箇所を修正
- [x] `src/semantic_analyzer/tests.rs` の `test_error_array_access_non_array` を正常系テストに変更
- [x] `resources/tests/fails/compile/not_an_array_001` を削除
- [x] 新テストケースを `resources/tests/passes/` に追加
  - `index_operator_non_array_001.ns`: 非配列変数への `[]` 読み取り
  - `index_operator_non_array_002.ns`: 非配列変数への `[]` 書き込み
  - `index_operator_non_array_003.ns`: `&x[i]` が参照演算と一致することの確認
- [x] テストを実行
  - Unit テスト: すべて成功
  - Large テスト: 新規追加の3テストが失敗

### 失敗したテスト

新規に追加した以下のテストが失敗:
- `test_index_operator_non_array_001` / `test_index_operator_non_array_001_ws_self`
- `test_index_operator_non_array_002` / `test_index_operator_non_array_002_ws_self`
- `test_index_operator_non_array_003` / `test_index_operator_non_array_003_ws_self`

失敗原因:
- `__clog` の出力が標準出力ではなく標準エラー出力に出力されている可能性
- または、期待値の設定に問題がある可能性

調査ドキュメント: `docs-ai/task/investigate-index-operator-tests.md` に記録
