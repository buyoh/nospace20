# Block Expression Implementation - Test Failure Investigation

**日付**: 2026-02-15  
**ステータス**: ✅ 完了

## 概要

ブロックスコープ式の実装後、4つの既存テストが失敗していた。
調査の結果、すべてブロック式実装とは無関係の既存テストデータのバグであることが判明し、修正を行った。

## 失敗したテスト

1. `test_functions_func_redefine_001`
2. `test_scope_func_shadowing_global_001`
3. `test_scope_func_shadowing_nested_001`
4. `test_scope_func_shadowing_siblings_001`

## 調査結果

### ブロック式実装前から既に失敗していた

`git checkout ce79ca1^` (ブロック式実装前) でも4テストすべてが失敗していることを確認。
ブロック式の実装は無関係。

### 原因1: func_shadowing テスト (3件) - check.json の記載ミス

check.json の `trace` 配列の値がトレースポイントのインデックス (`[0, 1, 2, 3]`) になっていたが、
正しくは各トレースポイントの実行回数 (`[1, 1, 1, 1]`) であるべきだった。

**修正内容**:
- `func_shadowing_global_001.check.json`: `[0, 1, 2, 3]` → `[1, 1, 1, 1]`
- `func_shadowing_nested_001.check.json`: `[0, 1, 2, 3, 4]` → `[1, 1, 1, 1, 1]`
- `func_shadowing_siblings_001.check.json`: `[0, 1, 2, 3, 4]` → `[1, 1, 1, 1, 1]`

### 原因2: func_redefine_001 - 重複定義チェック導入との矛盾

テストは同名関数の再定義（後の定義が上書き）を期待していたが、
`bb17755` (Fix duplicated defines) で重複定義チェックが導入された後、
`syntactic_analyze` がエラーを返すようになった。

既に `func_duplicate_global_001` が同じシナリオのコンパイルエラーテストとして存在していた。

**修正内容**:
- テストタイプを `success` → `compile_error` に変更
- テストファイルを `passes/functions/` → `fails/compile/` に移動
- マニフェストのパスとコメントを更新

## 修正後の全テスト結果

127 passed, 0 failed, 21 ignored
