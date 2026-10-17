# WASM `compile()` が peephole 最適化を適用せず、最適化パス表が 3 箇所に重複

- 重要度: **medium**
- 状態: 未着手

## 進捗

- [ ] 最小修正（optimization を渡す）
- [ ] パス名定義の一元化

## 問題点

- `src/wasm_api/api.rs:77-82`: `WsCompileOptions { ..Default::default() }` で optimization を捨てている
- `src/wasm_api/pipeline.rs:51-86`: `"peephole"` を受け付けない
- `src/wasm_api/api.rs:141-147`: `getOptions` に `peephole` がない
- `src/wasm_api/whitespace_vm.rs:101-106`: `WasmWhitespaceVM::new` は opt passes を受け取らない
- 参考: `src/cli_utils.rs:44-59,101-117`（CLI は `optimization: opt_options.clone()` を渡す）

`["all"]` 指定時 `OptimizationOptions::all()`（peephole=true）は作られるが `compile_to_ws` に渡されず、CLI と Web で出力が異なる。パス名の表は CLI enum・`parse_opt_passes`・`get_options` の 3 箇所で別々に保守され、既に食い違っている。`api.rs:55` の `_language_std` は検証後未使用。

## 違反している原則

- DRY / Single Source of Truth

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `api.rs` で `optimization: opt_options` を渡し、`parse_opt_passes` と `get_options` に `"peephole"` を追加 | `wasm_api` のみ |
| B | `OptimizationOptions` に `from_pass_name(&str)` と `PASS_NAMES: &[&str]` を定義し、CLI・WASM 双方から参照 | `optimizer/mod.rs`、`cli_utils.rs`、`wasm_api`、Web UI 側の選択肢表示 |

推奨: A を即時、B で再発防止。
