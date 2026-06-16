# WASM Build Phase 0/1/A/3 完了レポート

**完了日:** 2026-02-12  
**タスク:** `wasm-build` の基本機能実装

## 完了フェーズ

### Phase 0: ビルド基盤（共通）

✅ **完了 (2026-02-10)**

- [x] `Cargo.toml` に `wasm-bindgen` 等の依存を追加（feature flag `wasm`）
- [x] `lib` セクションに `cdylib` crate-type を追加
- [x] `wasm32-unknown-unknown` で `lib.rs` がコンパイルできることを確認
- [x] `Environment::new()` の `std::io::stdin()`/`stdout()` 問題に対応

### Phase 1: 基本 WASM API（run / compile）

✅ **完了 (2026-02-10)**

- [x] `src/wasm_api.rs` モジュール作成（`#[wasm_bindgen]` エクスポート）
- [x] `run(source, stdin)` 関数実装（CLI `--mode=run` 相当）
- [x] `compile(source, target, std)` 関数実装（CLI `--mode=compile` 相当）
- [x] エラー情報の JS 向けシリアライズ
- [x] `wasm-pack build` で `.wasm` + JS グルーコード生成を確認

### Phase A: Whitespace コンパイル + ステップ実行

✅ **完了 (2026-02-10)**

nospace → Whitespace コンパイル + Whitespace VM ステップ実行の WASM API。

- [x] WhitespaceVM の軽微な拡張（`pc()`, `call_stack_depth()` 等）
- [x] `compile_to_whitespace()` / `compile_to_mnemonic()` WASM API
- [x] `WasmWhitespaceVM` ステートフルラッパー実装
- [x] Node.js / ブラウザでのスモークテスト

詳細: [phase-a-report.md](phase-a-report.md)

### Phase 3: テスト・統合

✅ **基本機能完了 (2026-02-12)**

Node.js でのスモークテスト・既存テストケースの WASM 経由実行。

- [x] テスト環境構築（`tools/wasm-test/` ディレクトリ、package.json）
- [x] Node.js スモークテスト実装（`run` / `compile` / `parse` 関数）
- [x] WasmWhitespaceVM ステップ実行テスト実装
- [x] エラーケーステスト実装
- [x] README.md にテスト実行方法を追記
- [x] 全テスト通過確認（2026-02-12）

詳細: [wasm-build-phase3-nodejs-test.md](wasm-build-phase3-nodejs-test.md)

## 実装された機能

### WASM API (src/wasm_api.rs)

1. **基本実行・コンパイル API**
   - `run(source, stdin, debug)` — nospace コード実行
   - `compile(source, target, std)` — nospace → Whitespace/Mnemonic コンパイル
   - `parse(source)` — 構文チェック

2. **Whitespace VM ステップ実行 API**
   - `WasmWhitespaceVM::new(source, stdin)` — nospace から VM 作成
   - `WasmWhitespaceVM::from_whitespace(ws, stdin)` — Whitespace から VM 作成
   - `step(n)` — n ステップ実行
   - `get_stdout()`, `get_stack()`, `get_heap()` 等のデバッグ情報取得

### ビルド設定

- `Cargo.toml` に `wasm` feature flag 追加
- `cdylib` crate-type 設定
- `wasm-bindgen`, `serde-wasm-bindgen` 依存追加

### テスト

- `tools/wasm-test/test.mjs` に Node.js スモークテスト実装
- 全テストケース通過確認済み

## 未完了タスク

- [ ] **サイズ最適化** — 引き続き `wasm-build` タスクとして継続
  - `wasm-opt` による最適化
  - 不要機能の除外
  - LTO, strip 等のコンパイルオプション調整

## Phase B: nospace ステップ実行インタプリタ

**→ [suspendable-interpreter タスク](../task/suspendable-interpreter/) Phase 5 へ移動**

nospace を直接ステップ実行する中断可能インタプリタの WASM API は、
`suspendable-interpreter` タスクの Phase 1〜4 完了後に Phase 5 として実装されます。

## 関連ドキュメント

- [wasm-build/build-config.md](../task/wasm-build/build-config.md) — Cargo.toml・ビルド設定
- [wasm-build/api-design.md](../task/wasm-build/api-design.md) — WASM API 設計
- [wasm-build/implementation.md](../task/wasm-build/implementation.md) — 実装手順詳細
- [wasm-build/nodejs-test.md](../task/wasm-build/nodejs-test.md) — Node.js テスト設計

## ビルド・テスト方法

### WebAssembly ビルド

```bash
# wasm32 ターゲット追加（初回のみ）
rustup target add wasm32-unknown-unknown

# WASM ビルド
wasm-pack build --target bundler --features wasm
```

### Node.js テスト実行

```bash
cd tools/wasm-test && node test.mjs
```

---

**次のステップ:** [wasm-build タスク](../task/wasm-build/) でサイズ最適化の実装
