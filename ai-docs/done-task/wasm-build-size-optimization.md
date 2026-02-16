# WASM ビルド サイズ最適化 完了レポート

**完了日:** 2026-02-16  
**タスク:** wasm-build - WASM バイナリのサイズ最適化

## 概要

Rust 製 nospace インタプリタ・コンパイラの WASM ビルドにおいて、
バイナリサイズを削減し、Web 配信に適したサイズを実現した。

## 最適化結果

### サイズ比較

| 段階 | サイズ | 削減率 |
|------|--------|--------|
| 最初（最適化なし） | 269 KB | - |
| Cargo.toml 設定後 | 217 KB | 19% |
| wasm-opt 適用後 | **198 KB** | **26%** |
| gzip 圧縮後 | **78.3 KB** | **71%** |

**目標達成:** 500KB 以下を大幅にクリア（目標の 40% のサイズ）

### 実施した最適化

#### 1. Cargo.toml `[profile.release]` 設定

```toml
[profile.release]
opt-level = "z"     # サイズ最適化優先
lto = true          # Link Time Optimization 有効化
codegen-units = 1   # 並列コンパイル無効（最適化優先）
strip = true        # デバッグシンボル除去
panic = "abort"     # パニックハンドラのコード削減
```

**効果:** 269KB → 217KB (19% 削減)

#### 2. wasm-opt による後処理

```toml
[package.metadata.wasm-pack.profile.release]
wasm-opt = ["-Oz", "--enable-bulk-memory"]
```

- `-Oz`: 積極的なサイズ最適化
- `--enable-bulk-memory`: バルクメモリ操作のサポート（`memory.copy` 等）

**効果:** 217KB → 198KB (追加で 9% 削減)

## 修正したコード

### src/wasm_api.rs

1. **コンパイルエラー修正**
   - `fn convert_errors(...) -> JsResultErr` を `-> JsValue` に修正
   - 未定義の型 `JsResultErr` を使用していた問題を解決

2. **未使用インポート削除**
   - `use crate::whitespace::RuntimeError` を削除（未使用警告）

## テスト結果

**全 WASM テスト通過** (2026-02-16)

```bash
cd tools/wasm-test && node test.mjs
# WASM Node.js tests passed.
```

## ビルド方法

### 開発ビルド（デバッグ情報込み）

```bash
wasm-pack build --target bundler --features wasm --dev
```

### リリースビルド（最適化）

```bash
wasm-pack build --target bundler --features wasm
```

## 技術的詳細

### opt-level の選択

- `opt-level = "z"` を選択（`"s"` より積極的なサイズ最適化）
- 実行速度よりもサイズを優先
- Web 配信では初回ロード時間が重要なため、サイズ優先が適切

### LTO (Link Time Optimization)

- `lto = true` により、クレート間の最適化を有効化
- インライン化の機会が増加し、デッドコード削除が効果的に働く

### wasm-opt の --enable-bulk-memory

- Rust の `std` が生成する `memory.copy` 命令に対応するため必須
- このフラグがないと wasm-opt がエラーで失敗する

## 今後の改善可能性

さらなるサイズ削減を目指す場合:

1. **不要機能の条件コンパイル**
   - `#[cfg(not(target_arch = "wasm32"))]` で WASM 非対応機能を除外
   - デバッグ用トレース機能の条件コンパイル

2. **依存クレートの最小化**
   - 未使用の feature flags を無効化
   - より軽量な代替クレートの検討

3. **ABIgen の最適化**
   - `wasm-bindgen` の生成コードを最小化
   - 不要な TypeScript 型定義の削除

## 関連ドキュメント

- [wasm-build/build-config.md](../task/wasm-build/build-config.md) — Cargo.toml 設定詳細
- [wasm-build/api-design.md](../task/wasm-build/api-design.md) — WASM API 設計
- [wasm-build-phase0-1-a-3-completion.md](wasm-build-phase0-1-a-3-completion.md) — 基本機能完了レポート

## まとめ

- ✅ **26% のサイズ削減** (269KB → 198KB)
- ✅ **gzip 圧縮で 71% 削減** (269KB → 78.3KB)
- ✅ **目標 500KB 以下を達成** (目標の 40%)
- ✅ **全テスト通過** (機能に影響なし)

WASM ビルドのサイズ最適化は完了し、Web 配信に適した小さなバイナリサイズを実現した。
