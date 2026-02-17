# ステップ5: function_static_storage のインデックスキー化

親ドキュメント: [README.md](README.md)  
作成日: 2026-02-17

## 概要

`Environment.function_static_storage` のキーを `String`（関数名）から `usize`（関数インデックス）に変更する。
これにより、ランタイムでの文字列ベースの検索を排除し、`Scope.function_names` への依存を軽減する。

## 現状の問題

### バグ: キーの不整合

現在、`function_static_storage` のキーが初期化時とランタイム時で一致していない。

**初期化時** (`src/interpreter/mod.rs` L115):
```rust
let func_name = &scope.function_names[func_idx];
env.function_static_storage.insert(func_name.clone(), storage);
// キー例: "foo", "bar"
```

**ランタイム時** (`src/interpreter/exec.rs` L224):
```rust
let func_key = format!("__func_{}_{}", func_ref.scope_depth, func_ref.local_index);
if let Some(storage) = self.env.function_static_storage.get(&func_key) {
// キー例: "__func_0_2"
```

初期化で `"foo"` として格納した値が、ランタイムで `"__func_0_2"` として検索されるため、
**static 変数の初期化値がランタイムで復元されない**。

このバグはステップ5でキーを統一的に `usize` にすることで解消される。

## 変更計画

### 変更量: 小（3ファイルの軽微な修正）

### 1. `src/interpreter/environment.rs`

**変更内容**: `function_static_storage` の型を変更

```rust
// Before
pub(crate) function_static_storage: BTreeMap<String, Vec<i64>>,

// After
pub(crate) function_static_storage: BTreeMap<usize, Vec<i64>>,
```

影響箇所:
- `Environment::new()` — 変更不要（`BTreeMap::new()` は型推論で対応）
- `Environment::new_with_buffers()` — 同上
- `Environment::new_with_config()` — 同上

### 2. `src/interpreter/mod.rs` (`initialize_function_statics`)

**変更内容**: キーを関数名から関数インデックスに変更

```rust
// Before
let func_name = &scope.function_names[func_idx];
env.function_static_storage.insert(func_name.clone(), storage);

// After
env.function_static_storage.insert(func_idx, storage);
```

`scope.function_names` への参照がこの関数内で不要になる。

### 3. `src/interpreter/exec.rs` (ユーザー関数呼び出し)

**変更内容**: キーを `format!` 文字列から関数インデックスに変更

```rust
// Before
let func_key = format!("__func_{}_{}", func_ref.scope_depth, func_ref.local_index);
if has_static {
    if let Some(storage) = self.env.function_static_storage.get(&func_key) {

// After
let func_key = func_ref.local_index;
if has_static {
    if let Some(storage) = self.env.function_static_storage.get(&func_key) {
```

保存側も同様:
```rust
// Before
self.env.function_static_storage.insert(func_key, scope_data);

// After (変更なし — func_key の型が usize に変わるだけ)
self.env.function_static_storage.insert(func_key, scope_data);
```

## キーの一意性について

現在の設計ではすべての関数がルートスコープにフラット化されており（Phase 5 で実装済み）、
`func_ref.local_index` はグローバル関数リスト内での一意なインデックスである。
したがって `func_ref.local_index` をキーとして使用するだけで一意性が保証される。

`func_ref.scope_depth` は常に `0` であるため、キーに含める必要はない。

## テスト方針

- 既存の static 変数テスト（`resources/tests/` 配下）がそのまま回帰テストとして機能する
- バグ修正により、static 変数の初期化値が正しく復元されるようになるため、
  既存テストの結果が改善する可能性がある
- 追加テスト: static 変数の初期値がランタイムで正しく参照されることを確認するテストケース

## 影響範囲

| モジュール | 影響 |
|-----------|------|
| `interpreter/environment.rs` | 型変更 |
| `interpreter/mod.rs` | キー変更 |
| `interpreter/exec.rs` | キー変更 |
| `compiler_ws/` | 影響なし（function_static_storage を使用しない） |
| `semantic_analyzer/` | 影響なし |
| テストコード | 回帰テストのみ |

## 完了条件

- [ ] `function_static_storage` の型が `BTreeMap<usize, Vec<i64>>` に変更されている
- [ ] 初期化時とランタイム時で同じキー（関数インデックス）を使用している
- [ ] `cargo test` が全て PASS
- [ ] static 変数の初期化値がランタイムで正しく復元される
