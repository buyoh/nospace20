# 巨大モジュールの分割・責務分離

## 進捗サマリ (2026-03-01)

| モジュール | 元の行数 | 現在の行数 | 状態 |
|------------|---------|-----------|------|
| `semantic_analyzer/mod.rs` | 1801 | 326 | **完了** — `alias.rs`, `constexpr.rs`, `scope.rs`, `template.rs`, `types.rs`, `return_analysis.rs`, `context.rs`, `expression.rs`, `statement.rs` 分割済み |
| `wasm_api.rs` | 834 | 833 | 未着手 |
| `compiler_ws/expression.rs` | 1020 | 941 | 部分完了 — Store 統合・比較演算子データ駆動化済み。`expression_builtin.rs` 分離は任意 |
| `compiler_ws/alloc_runtime.rs` | 1713 | ディレクトリ化 | **完了** — `alloc_runtime/{mod.rs, bump.rs, fsba.rs}` に分割済み |

## 残タスク

### wasm_api.rs — 833 行（未着手）

JavaScript API 定義、パーサヘルパー、TypeScript 型定義、WhitespaceVM ラッパーが混在。

#### 改善案

```
src/wasm_api/
├── mod.rs              # #[wasm_bindgen] の高レベルエントリポイント
├── pipeline.rs         # 共通コンパイルパイプライン (トークン化→構文解析→意味解析)
├── run.rs              # run API
├── compile.rs          # compile API
├── parse.rs            # parse API
├── whitespace_vm.rs    # WasmWhitespaceVM ラッパー
└── types.rs            # TypeScript 型定義 (serde 構造体)
```

## 完了済み

### semantic_analyzer/mod.rs — 完了 (2026-03-01)

`mod.rs`（1127行）→ 以下のファイルに分割済み:
- `mod.rs`（326行）: analyze() エントリポイント + pub 型の再エクスポート
- `context.rs`（54行）: 解析コンテキスト（AnalyzeContext — analyze_internal_with_parent の引数を構造体化）
- `expression.rs`（462行）: 式の変換（ExecExpression 生成、convert_to_exec_expression_with_resolver）
- `statement.rs`（400行）: 文の変換（ExecStatement 生成、convert_to_exec_statements）
- `scope.rs`, `constexpr.rs`, `template.rs`, `alias.rs`, `types.rs`, `return_analysis.rs`（分割済み）

### compiler_ws/alloc_runtime — 完了

`alloc_runtime.rs`（1713行）→ `alloc_runtime/` ディレクトリ分割済み:
- `mod.rs`（230行）: AllocRuntime trait + ファクトリ
- `bump.rs`（372行）: BumpAllocRuntime
- `fsba.rs`（1102行）: FsbaAllocRuntime

### compiler_ws/expression.rs — 部分完了

1020行 → 941行（直前の refactor コミット `bf10475`）:
- Store/void 統合 (`emit_retrieve: bool`)
- 比較演算子データ駆動化 (`ComparisonSpec`)
- アドレス計算の委譲

## 優先度（残タスク）

| モジュール | 優先度 | 理由 |
|------------|--------|------|
| wasm_api.rs | Medium | 833 行、コンパイルパイプライン重複 |
| expression.rs リファクタリング | 中 | 任意 |
