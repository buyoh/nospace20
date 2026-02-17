# Bug D: 実装進捗

## 実施日

2026-02-17

## 実装内容

### 変更ファイル

1. `src/compiler_ws/context.rs`
   - `scope_offsets: Vec<i64>` フィールドを追加
   - `next_var_offset: i64` フィールドを追加
   - `new()` と `new_with_options()` メソッドで初期化
   - `enter_function()` メソッドを修正して `total_var_count` と `func_scope_var_count` を受け取る
   - `enter_block_scope()` と `leave_block_scope()` メソッドを追加
   - `get_var_info()` メソッドを修正して `scope_depth` を考慮
   - グローバル変数のスコープ深度オーバーフローに対する保護を追加

2. `src/compiler_ws/statement.rs`
   - `calculate_total_variable_count()` 関数を追加
   - `count_nested_vars_in_statements()` 関数を追加
   - `count_nested_vars_in_statement()` 関数を追加
   - `count_nested_vars_in_expression()` 関数を追加
   - `generate_function_definition()` を修正して全変数合計数を計算
   - `generate_block()` を修正してスコープ進入/退出時にオフセットを操作

## テスト結果

### 実行コマンド

```bash
cargo test --release --test code_test
```

### 結果サマリ

- **修正前**: 269 passed; 8 failed; 120 ignored
- **修正後**: 273 passed; 4 failed; 120 ignored

**改善**: 4 つのテストが新たに成功

### 成功したテスト（期待通り）

以下のテストが Bug D 修正により成功に変わりました:

- ✅ `test_example_qsort_ws_self` - メインのバグケース
- ✅ `test_scope_block_var_no_collision_001_ws_self` - ブロック変数衝突テスト
- ✅ その他複数のスコープ関連テスト

### 残りの失敗テスト（期待通り）

以下の 4 つのテストは引き続き失敗していますが、これは Bug D とは別の根本原因（if/while 式の値返却が未実装）によるものです:

1. `test_control_flow_if_expr_value_001_ws_self`
2. `test_control_flow_while_expr_value_001_ws_self`
3. `test_scope_block_expr_nested_001_ws_self`
4. `test_scope_block_expr_value_001_ws_self`

これらのテストの失敗は設計書で予測されていた通りです。

### 意図しない変更

静的変数 (static) とネストされた関数のスコープ問題に関連するテストが以前失敗していましたが、今回の修正でも同様の理由で失敗する可能性がありました。しかし、`get_var_info()` でスコープ深度のオーバーフローに対する保護を追加したことで、これらのテストも成功するようになりました。

## 結論

Bug D の修正は成功しました。内部ブロックスコープの変数が関数スコープの変数とヒープメモリアドレスを衝突する問題が解決され、期待された通りのテスト改善が確認されました。

残りの失敗テストは、別の機能（ブロック式の値返却）の未実装によるものであり、Bug D とは無関係です。
