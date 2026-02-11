# Rust → WebAssembly ビルド (ランタイム WASM 化)

## 概要

Rust 製 nospace インタプリタ・コンパイラ自体を `wasm32-unknown-unknown` ターゲットにビルドし、
JavaScript / ブラウザ / Node.js から CLI と同等の機能を呼び出せるようにする。

**既存タスク `wasm-js-compiler/` との違い:**

| | wasm-js-compiler (Approach A) | 本タスク (Approach B) |
|---|---|---|
| 方針 | nospace コードを WASM/JS コードに変換 | Rust 製ランタイム自体を WASM にビルド |
| 出力 | 小さな .wasm/.js | ランタイム込みの .wasm (数百KB〜) |
| 実装 | 新しいコンパイラバックエンド | wasm-bindgen でラップ |
| 機能カバー | 要個別実装 | 自動的に全機能対応 |

## ドキュメント

| ファイル | 内容 |
|---------|------|
| [build-config.md](build-config.md) | Cargo.toml・ビルド設定の変更 |
| [api-design.md](api-design.md) | WASM 公開 API の設計（run / compile） |
| [implementation.md](implementation.md) | 実装手順（Phase 0/1/A/B/3 全て含む統合ドキュメント） |
| [nodejs-test.md](nodejs-test.md) | Node.js WASM テスト設計（Phase 3） |

## 全体フェーズ計画

### Phase 0: ビルド基盤（共通）

- [ ] `Cargo.toml` に `wasm-bindgen` 等の依存を追加（feature flag `wasm`）
- [ ] `lib` セクションに `cdylib` crate-type を追加
- [ ] `wasm32-unknown-unknown` で `lib.rs` がコンパイルできることを確認
- [ ] `Environment::new()` の `std::io::stdin()`/`stdout()` 問題に対応

### Phase 1: 基本 WASM API（run / compile）

- [ ] `src/wasm_api.rs` モジュール作成（`#[wasm_bindgen]` エクスポート）
- [ ] `run(source, stdin)` 関数実装（CLI `--mode=run` 相当）
- [ ] `compile(source, target, std)` 関数実装（CLI `--mode=compile` 相当）
- [ ] エラー情報の JS 向けシリアライズ
- [ ] `wasm-pack build` で `.wasm` + JS グルーコード生成を確認

### Phase A: Whitespace コンパイル + ステップ実行 → [implementation.md](implementation.md#phase-a-whitespace-コンパイル--ステップ実行-api)

nospace → Whitespace コンパイル + Whitespace VM ステップ実行の WASM API。
既存の `compiler_ws` + `whitespace::interpreter` を活用。

**完了 (2026-02-10)** - 詳細: [phase-a-report.md](../../done-task/phase-a-report.md)

- [x] WhitespaceVM の軽微な拡張（`pc()`, `call_stack_depth()` 等）
- [x] `compile_to_whitespace()` / `compile_to_mnemonic()` WASM API
- [x] `WasmWhitespaceVM` ステートフルラッパー実装
- [x] Node.js / ブラウザでのスモークテスト

### Phase B: nospace ステップ実行インタプリタ → [implementation.md](implementation.md#phase-b-nospace-ステップ実行インタプリタ-api)

nospace を直接ステップ実行する中断可能インタプリタの WASM API。
`suspendable-interpreter` タスクの完了が前提条件。

- [ ] `suspendable-interpreter` の実装（Phase 1〜4）
- [ ] `OwnedInterpreterSession` の実装（Scope 所有版）
- [ ] `WasmInterpreterSession` WASM API 実装
- [ ] デバッグ情報 API（変数・コールスタック）
- [ ] テスト・検証

### Phase 3: テスト・統合 → [wasm-build-phase3-nodejs-test.md](../../done-task/wasm-build-phase3-nodejs-test.md)

Node.js でのスモークテスト・既存テストケースの WASM 経由実行。

- [x] テスト環境構築（`tools/wasm-test/` ディレクトリ、package.json）
- [x] Node.js スモークテスト実装（`run` / `compile` / `parse` 関数）
- [x] WasmWhitespaceVM ステップ実行テスト実装
- [x] エラーケーステスト実装
- [x] README.md にテスト実行方法を追記
- [x] 全テスト通過確認（2026-02-12）
- [ ] サイズ最適化（`wasm-opt`、不要機能の除外）

**Phase 3 完了** (2026-02-12)

## 実装順序の推奨

```
Phase 0: ビルド基盤
    ↓
Phase 1: 基本 WASM API (run/compile)
    ↓
    ├→ Phase A: WS コンパイル + WS VM ステップ実行
    │    依存: compiler_ws ✅, whitespace-interpreter Phase1&2 ✅
    │    追加実装: 軽微（VM ラッパーのみ）
    │
    └→ Phase B: nospace ステップ実行インタプリタ
         依存: suspendable-interpreter（未着手、大規模）
         追加実装: 大規模（インタプリタ改修 + セッション API）
    ↓
Phase 3: テスト・統合
```

Phase A は依存タスクがほぼ完了済みのため先行して実装可能。
Phase B は suspendable-interpreter の実装が必要であり、工数が大きいため後回しにできる。

## 前提条件

- `rustup target add wasm32-unknown-unknown`
- `cargo install wasm-pack`（または `npx wasm-pack`）

## 関連タスク

- [../whitespace-interpreter/](../whitespace-interpreter/) — Whitespace VM（Phase A の依存）
- [../suspendable-interpreter/](../suspendable-interpreter/) — 中断可能インタプリタ（Phase B の依存）
- [../wasm-js-compiler/](../wasm-js-compiler/) — 別アプローチ: nospace → WASM/JS コンパイラバックエンド
