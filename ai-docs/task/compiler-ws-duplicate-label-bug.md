# コンパイラ生成コードの重複ラベルバグ

## 状況

Whitespace インタプリタで重複ラベル定義のエラー検出を実装した結果、以下の3つのテストが新たに失敗するようになった:

1. `test_scope_func_shadowing_global_001_ws_self`
   - エラー: `DuplicateLabel { label_id: 16, first_position: 44, second_position: 137 }`
   
2. `test_scope_func_shadowing_nested_001_ws_self`
   - エラー: `DuplicateLabel { label_id: 18, first_position: 108, second_position: 168 }`
   
3. `test_scope_func_shadowing_siblings_001_ws_self`
   - エラー: `DuplicateLabel { label_id: 18, first_position: 139, second_position: 168 }`

## 原因

nospace コンパイラ (`compiler_ws`) が、異なる関数スコープに対して同じラベル ID を生成している。
これは、関数のスコープシャドーイングの実装において、ラベル ID の一意性が保証されていないことが原因と考えられる。

## 影響範囲

- 関数スコープのシャドーイングに関連するテストケース
- ラベル ID が重複する可能性のあるコード生成パターン

## 対応方針

1. `compiler_ws` のラベル ID 生成ロジックを調査
2. ラベル ID の一意性を保証するよう修正
3. 失敗したテストが合格することを確認

## 関連ファイル

- `src/compiler_ws/context.rs` - ラベル ID 生成ロジック
- `src/compiler_ws/codegen.rs` - コード生成ロジック
- 失敗したテストケース:
  - `resources/tests/passes/scope/func_shadowing_global_001.ns`
  - `resources/tests/passes/scope/func_shadowing_nested_001.ns`
  - `resources/tests/passes/scope/func_shadowing_siblings_001.ns`

## 参考資料

- `ai-docs/done-task/fix-ws-self-label-duplication.md` - 過去の類似バグ修正
