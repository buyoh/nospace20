# 死にコードと誤解を招く実装

- 重要度: **low**
- 状態: 未着手

`unused-code-cleanup.md` で判断済みの `is_global` / `new_label_range` は除外。

## 進捗

- [ ] compiler_ws / interpreter / optimizer の死にコード整理
- [ ] compile_property / CompileTarget の整理

## 問題点

### バックエンド

- `src/compiler_ws/memory.rs:8-73`: `MemoryLayout` 構造体はテスト以外でインスタンス化されず、メソッドはすべて `#[allow(dead_code)]`。実際に使われているのは定数のみで、`heap_layout` に「後方互換」用の二重定義がある。
- `src/interpreter/exec.rs:15-43`: `random_uninit_value` / `create_uninit_vec` は `#[allow(dead_code)]` で未使用。乱数初期化の実体は `allocator.rs` + `algorithm::hash` にあり二重実装。
- `src/optimizer/constant_folding.rs:213-243`: 「部分的な簡約（一方が定数 0 or 1）」とコメントにあるが、実際は何も簡約せず同じ式を組み直すだけ。
- `src/optimizer/geti_opt.rs:51-55`: `optimize_block` は `block.scope.static_init_statements` を処理しない（constant_folding は `:54` で処理）。`:161` の `dummy_location` は未使用。

### フロントエンド・設定

- `src/compile_property.rs:52-91`: `ValidationError::UnsupportedStd` / `IncompatibleOptions` / `InvalidExtension` はどこからも生成されない。`IncompatibleOptions` のメッセージは存在しない `--std=ws` を案内。
- `CompileTarget::Json` は validate で弾かれる前提のため `src/bin/nospace20.rs:224` と `src/wasm_api/api.rs:75` で `unreachable!()` に依存（不正状態が型で表現可能）。

## 違反している原則

- 死にコードの排除、コメントと実装の一致、型による不正状態の排除

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `MemoryLayout` を定数のみの `mod heap_layout` に統合し構造体を削除 | `compiler_ws/memory.rs`、`memory_tests.rs` |
| B | exec.rs の uninit ヘルパーと `dummy_location` を削除 | `interpreter/exec.rs`、`optimizer/geti_opt.rs` |
| C | constant_folding の偽の部分簡約は `x+0`、`x*1` 等を実装するか削除。geti_opt の static_init 走査は他パスと共通の Scope ウォーカーを抽出して揃える | `optimizer/*` |
| D | 未使用 `ValidationError` variant を削除。`Json` を `CompileTarget` から外して CLI 層の enum に留め、`unreachable!` を削除 | `compile_property.rs`、`bin/nospace20.rs`、`wasm_api/api.rs` |
