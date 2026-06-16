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

`LabelAllocator::function_labels` が `HashMap<String, LabelId>` で関数名をキーにしていたため、
semantic analyzer によってフラット化された同名関数（シャドーイング）に対して同じラベル ID が返されていた。

例: グローバルスコープの `foo` (index=0) と `outer` 関数内のネストされた `foo` (index=2) が、
`get_or_create_function_label("foo")` の呼び出しで同一のラベルを共有していた。

## 修正内容

`function_labels` のキーを関数名 (`String`) から関数のグローバルインデックス (`usize`) に変更。
これにより、同名関数でも異なるインデックスを持つため、一意のラベルが割り当てられるようになった。

### 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `src/compiler_ws/label.rs` | `function_labels` のキーを `String` → `usize` に変更、関連メソッド更新 |
| `src/compiler_ws/context.rs` | ラッパーメソッドのシグネチャを `&str` → `usize` に変更 |
| `src/compiler_ws/expression.rs` | `func_ref.local_index` を直接ラベル取得に使用 |
| `src/compiler_ws/statement.rs` | `func_index` をラベル取得に使用 |
| `src/compiler_ws/builtin.rs` | `scope.main_function_index` 経由でラベル取得 |

### テスト

- `test_shadowed_function_labels_are_unique` を `label.rs` に追加
- 既存テストを関数インデックスベースに更新

## ステータス

- [x] 原因調査完了
- [x] 修正実装完了 (2026-02-18)
- [x] 全テスト合格確認 (503 passed; 0 failed)
