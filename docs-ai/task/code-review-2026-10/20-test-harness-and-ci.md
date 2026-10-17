# テストハーネス・CI の品質（wsc 不在時の偽の成功、wasm feature 未テスト等）

- 重要度: **low**
- 状態: 未着手

## 進捗

- [ ] wsc 不在時の扱い
- [ ] 生成テストの戻り値型修正
- [ ] CI に wasm feature のテスト・ビルド追加

## 問題点

1. **wsc 不在時の偽の成功**: `tests/code_test/whitespace_base.rs:12-16,49-53` は wsc がないと eprintln して return し、結果は成功扱い。`--ignored` で実行する CI に wsc がないと全件「成功」と表示される。
2. **実ファイル作成**: `tests/common/mod.rs:7,79` は `tempfile::NamedTempFile` で実ファイルを作り外部プロセス wsc を起動。AGENTS.md は「ファイルを作成するようなテストの作成は禁止」としており、wsc 連携の性質上やむを得ないがルールとの関係が明文化されていない。
3. **型の誤用**: 生成テストの戻り値 `std::fmt::Result`（`src_build/nospace_tests.rs:47,360`、`tests/code_test/error_base.rs:1`）は意味的に誤りで、常に `Ok(())` を返すだけ。
4. **CI が `--features wasm` をテストしない**: `.github/workflows/test.yml:25-32` は default feature（cli）のみ。`src/lib.rs:32-33` の `wasm_api` はコンパイルすらされず、`pipeline::tests` の 4 件は CI で一度も実行されない（手元で `cargo test --features wasm --lib wasm_api` は 4 件 pass）。wasm32 ビルドの破損は手動実行の deploy-web まで気付けない。

## 違反している原則

- テストの信頼性、AGENTS.md のテスト規則

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | wsc 不在時は `panic!("wsc not available")` で明示的に失敗（`#[ignore]` は維持） | `tests/code_test/whitespace_base.rs`、`tests/whitespace_direct_test/base.rs` |
| B | wsc 連携テストを AGENTS.md で例外として明記、または wsc の stdin 実行でファイル作成を回避できるか検討 | `AGENTS.md`、`tests/common/mod.rs` |
| C | 生成テストの戻り値を `()` にする | `src_build/*.rs`、`tests/code_test/*_base.rs` |
| D | test.yml に `cargo test --features wasm --lib wasm_api` と `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` を追加 | `.github/workflows/test.yml` のみ |
