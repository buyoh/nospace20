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
| [api-design.md](api-design.md) | WASM 公開 API の設計 |
| [implementation.md](implementation.md) | 実装手順・コード変更詳細 |

## フェーズ計画

### Phase 1: ビルド基盤

- [ ] `Cargo.toml` に `wasm-bindgen` 等の依存を追加（feature flag `wasm`）
- [ ] `lib` セクションに `cdylib` crate-type を追加
- [ ] `wasm32-unknown-unknown` で `lib.rs` がコンパイルできることを確認
- [ ] `Environment::new()` の `std::io::stdin()`/`stdout()` 問題に対応

### Phase 2: WASM API 実装

- [ ] `src/wasm_api.rs` モジュール作成（`#[wasm_bindgen]` エクスポート）
- [ ] `run(source, stdin)` 関数実装（CLI `--mode=run` 相当）
- [ ] `compile(source, target, std)` 関数実装（CLI `--mode=compile` 相当）
- [ ] エラー情報の JS 向けシリアライズ
- [ ] `wasm-pack build` で `.wasm` + JS グルーコード生成を確認

### Phase 3: テスト・検証

- [ ] Node.js でのスモークテスト（`run` / `compile` の動作確認）
- [ ] 既存テストケースの一部を WASM 経由で実行し結果照合
- [ ] サイズ最適化（`wasm-opt`、不要機能の除外）

## 前提条件

- `rustup target add wasm32-unknown-unknown`
- `cargo install wasm-pack`（または `npx wasm-pack`）
