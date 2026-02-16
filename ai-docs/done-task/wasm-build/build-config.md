# ビルド設定

## Cargo.toml 変更

### crate-type の追加

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

- `rlib`: 通常の Rust ライブラリ（テスト・native ビルド用、既存動作を維持）
- `cdylib`: wasm-pack が要求する動的ライブラリ形式

**注意**: `cdylib` を追加すると native ビルドでも `.so`/`.dylib` が生成されるが、
既存の `cargo build` / `cargo test` には影響しない。

### feature flag `wasm`

```toml
[features]
default = []
wasm = ["wasm-bindgen", "serde-wasm-bindgen"]

[dependencies]
wasm-bindgen = { version = "0.2", optional = true }
serde-wasm-bindgen = { version = "0.6", optional = true }

# 既存の依存はそのまま
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0.64"
# ...
```

`wasm` feature が有効な場合のみ `wasm-bindgen` 関連コードがコンパイルされる。
native ビルドは一切影響を受けない。

### wasm-bindgen のバージョン

`wasm-bindgen = "0.2"` は現在の安定版。
`serde-wasm-bindgen = "0.6"` は `JsValue` ↔ Rust 型の変換に使用。

## ビルドコマンド

### wasm-pack を使用（推奨）

以下のうち実際にテストするのは `wasm-pack build --target bundler --features wasm` のみ。

```bash
# Node.js 向け
wasm-pack build --target nodejs --features wasm

# ブラウザ向け（bundler 前提）
wasm-pack build --target bundler --features wasm

# ブラウザ向け（ES modules, bundler 不要）
wasm-pack build --target web --features wasm
```

出力先: `pkg/` ディレクトリ

```
pkg/
├── nospace20_bg.wasm      # WASM バイナリ
├── nospace20_bg.wasm.d.ts # TypeScript 型定義 (wasm)
├── nospace20.js           # JS グルーコード
├── nospace20.d.ts         # TypeScript 型定義
└── package.json           # npm パッケージ定義
```

### cargo で直接ビルド（低レベル）

```bash
cargo build --target wasm32-unknown-unknown --features wasm --lib
```

`wasm-pack` が推奨。wasm-bindgen の JS グルーコード生成を自動で行うため。

## native ビルドとの共存

feature flag + conditional compilation で native ビルドへの影響をゼロにする。

```rust
// src/wasm_api.rs  - feature gate
#[cfg(feature = "wasm")]
mod wasm_api;
```

```bash
# native ビルド（従来通り）
cargo build
cargo test

# wasm ビルド
wasm-pack build --features wasm
```

## .gitignore 追加

```
/pkg/
```

`wasm-pack build` の出力ディレクトリを除外。

## Environment::new() の対応

`Environment::new()` は `std::io::stdin()` / `std::io::stdout()` を使用しているが、
`wasm32-unknown-unknown` では以下の動作になる:

- `stdin`: `read()` が常に `Ok(0)`（EOF）を返す
- `stdout`: 出力が破棄される

**対応方針**: WASM API では `Environment::new()` を直接使わない。
代わに `Environment::new_with_buffers()` / `new_with_config()` を使い、
stdin/stdout を `Vec<u8>` ベースのバッファで差し替える。

`lib.rs` の `interpret_func_with_io()` が既にこのパターンを実装済みのため、
WASM API はこれを活用する。

既存の `Environment::new()` は変更しない（native CLI はそのまま使うため）。

## clap 依存の扱い

`clap` は `src/bin/nospace20.rs`（CLI バイナリ）でのみ使用。
`wasm-pack build --lib` はバイナリをビルドしないため、`clap` が wasm でコンパイルされることはない。
変更不要。
