# Whitespace コンパイラでのネストされた関数の static 変数問題

作成日: 2026-02-18  
ステータス: ✅ 完了

## 問題の概要

`test_scope_scope_static_mixed_001_ws_self` テストが失敗していた。  
親関数 `test()` 内の static 変数 `shared` に、ネストされた関数 `inner()` からアクセスすると失敗する。

エラー: `AssertionFailed(0)`

## 根本原因

`get_var_info()` で static 変数のグローバルオフセットを検索する際、
`(current_func_index, slot_index)` をキーとして使用していたが、
ネストされた関数からアクセスする場合、`current_func_index` はネスト関数自身のインデックスであり、
親関数のインデックスではないため、正しいオフセットが取得できなかった。

## 修正内容

`IdentifierRef` に `owning_func_index: Option<usize>` フィールドを追加し、
関数境界を越えた static 変数アクセス時に、変数を所有する関数のグローバルインデックスを記録するようにした。

### 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/semantic_analyzer/types.rs` | `IdentifierRef` に `owning_func_index` フィールド追加 |
| `src/semantic_analyzer/scope.rs` | `ScopeInfo` に `func_global_index` 追加。`enter_scope` に引数追加。`resolve_variable` で関数境界越え時に `owning_func_index` を設定 |
| `src/semantic_analyzer/mod.rs` | `analyze_internal_with_parent` に `func_global_index` 引数追加。関数宣言処理で関数インデックスを渡す |
| `src/compiler_ws/context.rs` | `get_var_info` で `owning_func_index` を使用して正しいオフセットを検索 |
| `src/interpreter/exec.rs` | テストコードの `IdentifierRef` 構築に `owning_func_index: None` 追加 |
