# Add runtime error tests for __assert and __assert_not failures

日付: 2026-02-17

## タスクの概要

`resources/tests/fails` に `__assert` の失敗ケースを追加する。

## 実施内容

### 1. 新しいディレクトリとテストケースの作成

- `resources/tests/fails/runtime/` ディレクトリを作成
- 以下の3つのテストケースを追加:
  - `assert_fail_001`: `__assert(0)` が panic することを確認
  - `assert_not_fail_001`: `__assert_not(1)` が panic することを確認
  - `assert_in_expr_001`: 式の中で `__assert(0)` が panic することを確認

### 2. テストインフラの実装

- `build.rs`: `runtime_error` テストタイプを追加
- `tests/code_test.rs`:
  - `TestConfig` enum に `RuntimeError` バリアントを追加
  - `test_runtime_error_base` 関数を実装（`std::panic::catch_unwind` を使用してパニックをキャッチ）
- `resources/tests/test-manifest.yaml`: runtime error テストエントリを追加

### 3. テスト実行結果

- 新しく追加した3つのテストはすべて成功: ✅
  - `test_runtime_error_assert_fail_001 ... ok`
  - `test_runtime_error_assert_not_fail_001 ... ok`
  - `test_runtime_error_assert_in_expr_001 ... ok`

- 全体のテスト結果: 268 passed; 2 failed (既存の ws_self target の既知の問題)

### 4. コミット

コミットID: `f39b2cc`
コミットメッセージ: "Add runtime error tests for __assert and __assert_not failures"

## 変更ファイル

- 新規作成:
  - `resources/tests/fails/runtime/assert_fail_001.ns`
  - `resources/tests/fails/runtime/assert_fail_001.check.json`
  - `resources/tests/fails/runtime/assert_not_fail_001.ns`
  - `resources/tests/fails/runtime/assert_not_fail_001.check.json`
  - `resources/tests/fails/runtime/assert_in_expr_001.ns`
  - `resources/tests/fails/runtime/assert_in_expr_001.check.json`

- 変更:
  - `build.rs`: runtime_error テストタイプのサポートを追加
  - `tests/code_test.rs`: RuntimeError バリアントと test_runtime_error_base 関数を追加
  - `resources/tests/test-manifest.yaml`: runtime error テストエントリを追加

## 技術的詳細

### __assert の動作

`src/interpreter/exec.rs` の実装より:
- `__assert(x)`: x が 0 の場合 `panic!("assertion failed: {} == 0", a)` を発生させる
- `__assert_not(x)`: x が 0 以外の場合 `panic!("assertion failed: {} != 0", a)` を発生させる

### runtime_error テストの実装

```rust
fn test_runtime_error_base(test_name: &str) -> Result {
    // パース -> セマンティクス分析
    // ...
    
    // 実行してパニックをキャッチ
    let result = std::panic::catch_unwind(|| {
        interpret_func_with_io(&a, "main", "");
    });

    assert!(result.is_err(), "Expected runtime panic but succeeded");
    
    // contains が指定されている場合、パニックメッセージに含まれているか確認
    // ...
}
```

## 完了条件

- [x] テストケースを追加
- [x] テストインフラを実装
- [x] テストを実行して成功を確認
- [x] 変更をコミット
- [x] ドキュメントを作成

## 備考

- 既存の2つの失敗テストは今回の変更とは無関係な既存の問題（ws_self target）
- `resources/tests/README.md` にはすでに `runtime/` ディレクトリの記載があったが、実際には存在していなかった
- 今回、初めて runtime error テストのインフラが整備された
