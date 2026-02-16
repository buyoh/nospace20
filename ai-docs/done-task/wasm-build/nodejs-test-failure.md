# Node.js WASM テスト失敗調査メモ

## 発生日時

2026-02-12

## 失敗内容

- 実行コマンド: `cd tools/wasm-test && node test.mjs`
- エラー: `Cannot find module '../../pkg/nospace20.js'`

## 直接の原因

`wasm-pack build --target nodejs --no-default-features --features wasm` が実行できておらず、`pkg/` が生成されていない。
(環境に `wasm-pack` がインストールされていないため。)

## 次のアクション

- `wasm-pack` をインストールする (`cargo install wasm-pack` もしくは `npx wasm-pack`)。
- `wasm-pack build --target nodejs --no-default-features --features wasm` を実行し、`pkg/` を生成する。
- 再度 `cd tools/wasm-test && node test.mjs` を実行し、結果を確認する。
