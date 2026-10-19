# 01: WASM `compile()` に最適化オプションを渡し、パス名定義を一元化する

作成日: 2026-10-19
ステータス: 完了

## 背景・動機

[12-wasm-compile-opt-mismatch.md](../code-review-2026-10/12-wasm-compile-opt-mismatch.md) 参照。

## 設計・方針

### A: 最小修正

- `wasm_api/api.rs` の `compile()` で `WsCompileOptions { optimization: opt_options, .. }` を渡す（`analyze_and_optimize` が返す 3 番目の値を使う）
- これにより `["all"]` で peephole が有効になり CLI と出力が一致する

### B: パス名の単一定義

`optimizer/mod.rs` に定義する。

```rust
/// 最適化パス名と、`OptimizationOptions` の該当フラグへのアクセサの対応表
/// （CLI の `--opt`・WASM の `opt_passes` で共通）
pub const PASS_NAMES: &[&str] = &["all", "condition-opt", "geti-opt", "constant-folding", "dead-code", "peephole"];

impl OptimizationOptions {
    /// パス名の列から構築する。"all" を含めば all()。未知の名前は Err(名前)
    pub fn from_pass_names<S: AsRef<str>>(names: &[S]) -> Result<Self, String>;
}
```

- WASM: `parse_opt_passes` は `from_pass_names` を呼び、エラーメッセージの候補一覧は `PASS_NAMES` から生成。`get_options` の `optPasses` は `PASS_NAMES`
- CLI: clap の `ValueEnum`（`CliOptPass`）は残す（ヘルプ表示に必要）。`build_optimization_options` は各値の `to_possible_value().get_name()` を `from_pass_names` に渡す。`CliOptPass` の名前集合と `PASS_NAMES` が一致することをユニットテストで保証する
- `noop_test_pass` はテスト用のため名前を持たない（従来どおり）

## テスト

- `optimizer` のユニットテスト: `from_pass_names` の各パス・"all"・空・未知の名前
- `cli_utils` のユニットテスト: `CliOptPass` の名前集合 == `PASS_NAMES`、`--opt peephole` で peephole が有効
- WASM API は wasm32 ターゲットでのみ動くため、`pipeline` の変換ロジックは `from_pass_names` のテストで担保する

## チェックリスト

1. [x] `PASS_NAMES` / `from_pass_names` 追加とテスト
2. [x] CLI を `from_pass_names` 経由に変更、名前一致テスト
3. [x] WASM `compile` / `parse_opt_passes` / `get_options` 修正
4. [x] `cargo check --features wasm --target wasm32-unknown-unknown`（可能なら）
