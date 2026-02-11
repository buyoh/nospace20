# Variable.identifier を slot_index に置き換える実装タスク

親タスク: [symbol-table-design.md](./symbol-table-design.md)

作成日: 2026-02-11

## 目的

`Variable` 構造体から文字列フィールド `identifier` を削除し、代わりに `slot_index: usize` を直接保持することで:
- ランタイムでの文字列検索を排除
- `Scope.variable_indices` マップを不要にする
- メモリ効率とパフォーマンスを向上

## 実装方針

### 1. Variable 構造体の変更

```rust
// 変更前
pub(crate) struct Variable {
    pub identifier: String,
    pub is_static: bool,
    pub array_size: Option<usize>,
}

// 変更後
pub(crate) struct Variable {
    pub slot_index: usize,
    pub is_static: bool,
    pub array_size: Option<usize>,
}
```

### 2. 影響範囲

#### (a) ScopeBuilder.build()
- `variable_indices` マップの構築ロジックを変更
- `Variable` 作成時に `slot_index` を計算して設定
- `variable_name_to_var_index` は semantic analyzer 内部で使用されるため残す

#### (b) interpreter/exec.rs
- L220, L289: `variable_indices[&var.identifier]` → `var.slot_index` に変更
- static 変数復元ロジックを簡素化

#### (c) Scope 構造体
- `variable_indices: BTreeMap<String, usize>` フィールドを削除可能
  ただし、他の箇所で使用されていないか確認が必要

### 3. 実装手順

1. `Variable` 構造体に `slot_index` フィールドを追加（`identifier` は一旦残す）
2. `ScopeBuilder.build()` で `slot_index` を設定
3. `exec.rs` で `var.slot_index` を使用するよう変更
4. テストを実行して動作確認
5. `Variable.identifier` フィールドを削除
6. `Scope.variable_indices` の削除可否を判断

## 進捗

### 2026-02-11: タスク開始

- 現在のコード構造を調査
- `Variable.identifier` の使用箇所:
  - `semantic_analyzer/scope.rs` L289, L290: マップ構築時
  - `semantic_analyzer/mod.rs` L325, L326: マップ構築時
  - `interpreter/exec.rs` L220, L289: static 変数復元時

次のステップ: すべての使用箇所を調査し、段階的に実装
