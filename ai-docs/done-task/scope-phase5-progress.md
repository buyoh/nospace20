# Phase 5 ネスト関数実装の進捗

## 日時

2026-02-10

## 実施内容

### 1. テストの追加

- 子スコープの関数にアクセスしようとするエラーテストを追加
  - `resources/tests/fails/scope/scope_nested_func_child_access_error_001.ns`
  - `resources/tests/fails/scope/scope_nested_func_child_access_error_001.check.json`

### 2. 核心の実装

#### 2.1 型定義の更新

- `ExecExpression` を更新：`Function` を `BuiltinFunction` と `UserFunction` に分離
- `Function` 構造体に `scope_depth` フィールドを追加
- `IdentifierInfo` と `Identifier` に `Clone` trait を実装
- `Scope::functions` を `pub(crate)` に変更（interpreter からアクセスするため）

#### 2.2 スコープ解決の更新

- `ScopeInfo` に `func_map` フィールドを追加
- `ScopeResolver::enter_scope` に `func_map` 引数を追加
- `ScopeResolver::resolve_function` メソッドを追加：ネスト関数の可視性チェック

#### 2.3 semantic_analyzer の更新

- 3パス解析に変更：
  - パス1a: 関数宣言を先にスキャンして登録（ホイスティング対応）
  - パス1b: 変数宣言収集
  - パス2: 変数と関数の初期化・本体を解析
- ネスト関数のサポートを有効化（エラーチェックを削除）
 - `Expression::Function` の処理を更新：組み込み関数とユーザー定義関数を区別
- 関数呼び出しを identifier resolution に変更

#### 2.4 interpreter の更新

- `interpret_expression` で `BuiltinFunction` と `UserFunction` を個別に処理
- `interpret_call_user_function_by_ref` メソッドを追加：`IdentifierRef` を使用してユーザー定義関数を呼び出す

#### 2.5 compiler_ws の更新

- `ExecExpression::BuiltinFunction` と `UserFunction` を処理
- ユーザー定義関数呼び出しは未サポート（エラーを返す）

### 3. test-manifest.yaml の更新

- 無効化されていたテストを有効化：
  - `scope_nested_func_001`
  - `scope_static_nested_001`
  - `scope_static_mixed_001`
  - `scope_static_multi_decl_001`
  - `scope_static_counter_factory_001`
  - `scope_static_error_001`
- 新しいテストを追加：
  - `scope_nested_func_child_access_error_001`

### 4. テストの修正

- `scope_nested_func_001.check.json` の trace を `[0, 1, 2]` に修正

## 現在の問題

### スタックオーバーフロー

ネスト関数を含むテストを実行すると、スタックオーバーフローが発生します。

**症状:**
- 簡単なテスト（ネスト関数なし）は成功
- ネスト関数を含むテストでスタックオーバーフローが発生

**可能性のある原因:**
1. パス1aで関数のプレースホルダーを作成する際、空の `Scope` を作成しているが、その `Scope` の `identifier_map` が再帰的に参照される可能性
2. `temporary_scope.identifier_map` に未完成の関数が含まれているため、resolver が無限ループに陥る可能性
3. 関数本体を解析する際に、再帰的に `analyze_internal_with_parent` が呼ばれ、無限ループが発生

**次のステップ:**
1. デバッグ出力を追加して、どこで無限ループが発生しているか特定
2. プレースホルダーの Scope 作成方法を見直す
3. 関数宣言のホイスティング処理を簡略化する（例：関数名だけを先に登録し、本体は後で解析）

## ビルド状況

- コンパイル: 成功（警告あり）
- 簡単なテスト（ネスト関数なし）: 成功
- ネスト関数を含むテスト: スタックオーバーフロー

## 次の作業

1. スタックオーバーフローの原因を特定
2. 修正を実装
3. 全テストを実行して結果を確認
4. 失敗したテストの調査ドキュメントを作成（必要に応じて）
5. 変更をコミット
