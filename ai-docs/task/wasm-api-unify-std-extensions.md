# WASM API: std_extensions パラメータの統一

## 概要

`getOptions()` が返す `stdExtensions: StdExtension[]` の形式と、`compile()` / `WasmWhitespaceVM` コンストラクタの `debug_ext: bool, alloc_ext: bool` 形式が不一致。
拡張は今後増加する可能性があるため、統一してスケーラブルにする。

## 現状

| API | std_extensions の受け取り方 |
|---|---|
| `getOptions()` | `{ stdExtensions: ["debug", "alloc"] }` (配列) |
| `compile()` | `debug_ext?: boolean, alloc_ext?: boolean` (個別bool) |
| `WasmWhitespaceVM::new()` | `debug_ext?: boolean, alloc_ext?: boolean` (個別bool) |

## 方針

WASM 側を修正し、`compile()` と `WasmWhitespaceVM::new()` で `StdExtension[]` 配列を受け取る形式に統一する。

### 変更後の TypeScript 型

```typescript
compile(source: string, target: CompileTarget, lang_std: LanguageStd, std_extensions?: StdExtension[] | null): CompileResult;

new WasmWhitespaceVM(source: string, stdin: string, interactive?: boolean | null, std_extensions?: StdExtension[] | null);
```

### Rust 実装

1. extern type `JsStdExtensionArray` を追加（`typescript_type = "StdExtension[]"`）
2. `parse_std_extensions()` ヘルパー関数で配列をパースし `(debug: bool, alloc: bool)` に変換
3. `compile()` と `WasmWhitespaceVM::new()` のシグネチャを変更

### 影響範囲

- `src/wasm_api.rs` のみ
- 内部 Rust API (`compile_to_whitespace_with_options` 等) は変更不要
- `run()` はインタプリタ実行のため影響なし
- pkg/ の `.d.ts` は WASM ビルドで再生成される

## ステータス

- [x] 設計
- [x] 実装
- [x] テスト確認
- [x] コミット
