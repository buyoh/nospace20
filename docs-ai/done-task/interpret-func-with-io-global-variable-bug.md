# interpret_func_with_io がグローバル変数を初期化しないバグ

## ステータス: 修正済み

## 問題

`test_legacy_006`, `test_legacy_011`, `test_legacy_012`, `test_legacy_013` をコメントアウト解除すると、以下のエラーで失敗する:

```
thread 'test_legacy_006' panicked at src/interpreter/exec.rs:76:38:
index out of bounds: the len is 0 but the index is 0
```

一方、`cargo run` で同じスクリプトを直接実行すると問題なく動作する。

## 原因

`src/lib.rs` の `interpret_func_with_io` 関数がグローバル変数の初期化を行っていない。

### 実行パスの違い

| 呼び出し元 | 使用する関数 | グローバル変数初期化 |
|---|---|---|
| `cargo run` (CLI) | `interpreter::interpret()` | **する** (`env.global_variables = vec![0; scope.variable_count]`) |
| `interpret_func_testing()` (`success` テスト) | `interpreter::interpret()` (main のとき) | **する** (Phase 3 で修正済み) |
| `interpret_func_with_io()` (`success_io` テスト) | `interpreter::interpret_func()` | **しない** ← バグ |

### 詳細

- `interpreter::interpret()` (`src/interpreter/mod.rs:31`) は `env.global_variables = vec![0; scope.variable_count]` でグローバル変数領域を確保してから `interpret_func` を呼ぶ
- `interpreter::interpret_func()` (`src/interpreter/mod.rs:18`) はグローバル変数の初期化を行わず、直接関数実行する
- `interpret_func_with_io()` (`src/lib.rs:91`) は `interpreter::interpret_func()` を直接呼んでいるため、`global_variables` が空の `Vec::new()` のまま
- グローバル変数にアクセスしようとすると `self.env.global_variables[id.local_index]` で index out of bounds が発生する

`interpret_func_testing()` (`src/lib.rs:73`) は Phase 3 で修正が入っており、`func_name == "main"` のとき `interpreter::interpret()` を使うようになっているが、`interpret_func_with_io()` には同等の修正が適用されていない。

## 修正方針

`src/lib.rs` の `interpret_func_with_io` 関数で、`interpret_func_testing` と同様に `func_name == "main"` の場合は `interpreter::interpret()` を使うようにする。

### 修正箇所

`src/lib.rs` の `interpret_func_with_io` 関数内（約 line 126）:

```rust
// 修正前:
interpreter::interpret_func(&mut env, scope, func_name);

// 修正後:
if func_name == "main" {
    interpreter::interpret(&mut env, scope);
} else {
    interpreter::interpret_func(&mut env, scope, func_name);
}
```
