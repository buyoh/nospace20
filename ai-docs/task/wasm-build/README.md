# Rust → WebAssembly ビルド (WASM サイズ最適化)

## 概要

Rust 製 nospace インタプリタ・コンパイラの WASM ビルドにおいて、
バイナリサイズを削減し、Web 配信に適した小さなファイルサイズを実現する。

**Phase 0/1/A/3 完了** (2026-02-12)  
基本的な WASM API（run/compile）、Whitespace VM ステップ実行、テスト環境は実装済み。  
詳細: [完了レポート](../../done-task/wasm-build-phase0-1-a-3-completion.md)

**サイズ最適化完了** (2026-02-16)  
Cargo.toml 設定 + wasm-opt により **269KB → 198KB (26% 削減)** を達成。  
gzip 圧縮後: **78.3 KB**

## 最適化結果

### サイズ比較

| 段階 | サイズ | 削減率 |
|------|--------|--------|
| 最初（最適化なし） | 269 KB | - |
| Cargo.toml 設定後 | 217 KB | 19% |
| wasm-opt 適用後 | **198 KB** | **26%** |
| gzip 圧縮後 | **78.3 KB** | **71%** |

### 実施した最適化

1. **Cargo.toml `[profile.release]` 設定**
   - `opt-level = "z"` - サイズ最適化優先
   - `lto = true` - Link Time Optimization
   - `codegen-units = 1` - 並列コンパイル無効（最適化優先）
   - `strip = true` - デバッグシンボル除去
   - `panic = "abort"` - パニックハンドラ削減

2. **wasm-opt による後処理**
   - `-Oz` フラグによる積極的なサイズ最適化
   - `--enable-bulk-memory` でバルクメモリ操作をサポート

### テスト結果

全ての WASM テスト通過確認済み（2026-02-16）:
```bash
cd tools/wasm-test && node test.mjs
# WASM Node.js tests passed.
```


## ドキュメント

| ファイル | 内容 |
|---------|------|
| [build-config.md](build-config.md) | Cargo.toml 設定詳細 |
| [api-design.md](api-design.md) | WASM API 設計 |
| [implementation.md](implementation.md) | Phase A/B 実装手順 |

## ビルド方法

### 開発ビルド（デバッグ情報込み）

```bash
wasm-pack build --target bundler --features wasm --dev
```

### リリースビルド（最適化）

```bash
wasm-pack build --target bundler --features wasm --release
```

### テスト実行

```bash
cd tools/wasm-test && node test.mjs
```

## 関連タスク

- [../suspendable-interpreter/](../suspendable-interpreter/) Phase 5 — nospace ステップ実行 WASM API
- [../../done-task/wasm-build-phase0-1-a-3-completion.md](../../done-task/wasm-build-phase0-1-a-3-completion.md) — Phase 0/1/A/3 完了レポート

