# 巨大モジュールの分割・責務分離

## 現状の問題

### semantic_analyzer/mod.rs — 1801 行

最も深刻な巨大モジュール。以下の責務が単一ファイルに混在している:

1. **constexpr 評価** — `evaluate_constexpr_expr`, `evaluate_constexpr_by_name`, `collect_constexpr_table`
2. **テンプレート展開** — テンプレート関数のインスタンス化処理
3. **alias 処理** — `collect_block_alias_refs_in_*` 系 4 関数
4. **式変換** — `convert_to_exec_expression_with_resolver` 等
5. **文変換** — `convert_to_exec_statement` 等
6. **識別子解決** — スコープ管理、変数/関数の解決
7. **メインエントリ** — `analyze`, `analyze_internal_with_parent`

#### 改善案

```
src/semantic_analyzer/
├── mod.rs              # analyze() エントリポイント + pub 型の再エクスポート
├── scope.rs            # Scope 構造体、識別子解決ロジック
├── expression.rs       # 式の変換（ExecExpression 生成）
├── statement.rs        # 文の変換（ExecStatement 生成）
├── constexpr.rs        # constexpr 関連（evaluate_constexpr_* + collect_constexpr_table）
├── template.rs         # テンプレート展開ロジック
├── alias.rs            # alias 参照収集（collect_block_alias_refs_in_*）
└── context.rs          # 解析コンテキスト（analyze_internal_with_parent の引数8つを構造体化）
```

#### `analyze_internal_with_parent` の引数過多

現在の引数（推定 8 個）を以下のコンテキスト構造体に集約:

```rust
struct AnalyzeContext<'a> {
    parent_scope: Option<&'a Scope>,
    constexpr_table: &'a ConstexprTable,
    alias_map: &'a AliasMap,
    block_alias_map: &'a BlockAliasMap,
    template_instances: &'a mut Vec<TemplateInstance>,
    errors: &'a mut Vec<CodeParseError>,
    // ...
}
```

### wasm_api.rs — 834 行

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

### compiler_ws/expression.rs — 1020 行

式のコード生成。Global/Local 分岐パターンの繰り返しにより肥大化。

#### 改善案

- Store/Retrieve の void context パラメータ化で約 200 行削減
- Global/Local アドレス解決をヘルパー関数に抽出
- 比較演算子 6 種のコード生成をデータ駆動に変換

### compiler_ws/alloc_runtime.rs — 1713 行

メモリアロケータのランタイムコード生成。Whitespace 命令列がインラインで記述。

#### 改善案

```
src/compiler_ws/
├── alloc_runtime/
│   ├── mod.rs          # AllocRuntime trait + ファクトリ
│   ├── bump.rs         # BumpAllocRuntime
│   └── fsba.rs         # FsbaAllocRuntime
```

## 優先度

| モジュール | 優先度 | 理由 |
|------------|--------|------|
| semantic_analyzer | High | 1801 行、最も変更頻度が高い |
| wasm_api | Medium | 834 行、コンパイルパイプライン重複 |
| expression.rs | Medium | 1020 行、パターン重複が多い |
| alloc_runtime.rs | Low | 1713 行だが変更頻度は低い |

## 作業見積もり

- semantic_analyzer 分割: 大（テスト整備含む）
- wasm_api 分割: 中
- expression.rs リファクタリング: 中
- alloc_runtime.rs 分割: 小（ファイル分割のみ）
