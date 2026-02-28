# wasm_api 分割設計

## 進捗

- **ステータス**: 完了
- **完了日**: 2026-03-01
- **実装内容**:
  - `src/wasm_api.rs`（834行）を `src/wasm_api/` ディレクトリに分割
  - `types.rs`: TS型定義・Serde構造体・`ResultErr::single_error` ヘルパー
  - `pipeline.rs`: 共通パイプライン（`analyze_source`, `analyze_and_optimize`）・パラメータパーサ・エラー変換
  - `api.rs`: `run`, `compile`, `parse`, ヘルパー関数・メタデータ
  - `whitespace_vm.rs`: `SharedWriter`, `WasmWhitespaceVM`, `create_from_ws_source` ヘルパー
  - 4箇所の重複パイプラインを `analyze_source` / `analyze_and_optimize` に統一
  - 7箇所の手動 `ResultErr` 構築を `ResultErr::single_error()` に統一
  - `from_whitespace` / `from_whitespace_interactive` の重複を `create_from_ws_source` ヘルパーで解消
  - `cargo test`: 全40テスト通過
  - `cargo build --features wasm`: 警告なしでビルド成功

## 現状

[src/wasm_api.rs](../../../src/wasm_api.rs) は 834 行の単一ファイルで、以下の責務が混在:

- TypeScript 型定義 (L22–116)
- Serde 結果構造体 (L118–145)
- JS パラメータパーサ (L147–248)
- エラー変換 (L250–273)
- トップレベル API: `run`, `compile`, `parse` (L275–510)
- Whitespace VM ラッパー: `WasmWhitespaceVM` + 16 メソッド (L514–800)
- ヘルパー関数・メタデータ (L803–834)

## コード重複の分析

### 重複 1: コンパイルパイプライン（最大の問題）

`parse_to_tokens → parse_to_tree → syntactic_analyze` の同一パイプラインが **4 箇所** で展開:

| 箇所 | 行 | エラー返却方式 | 追加ステップ |
|------|-----|---------------|-------------|
| `run()` | L290–303 | `.into()` | + optimize + interpret |
| `compile()` | L432–444 | `.into()` | + optimize + compile_to_ws |
| `parse()` | L484–497 | `.into()` | なし |
| `WasmWhitespaceVM::new()` | L573–585 | `Err(...)` | + compile_to_ws |

### 重複 2: `ResultErr` インライン構築 — 7 箇所

`ResultErr { success: false, errors: vec![WasmError { ... }] }` の手動構築が散在。

### 重複 3: `WhitespaceVM::from_source` エラー処理 — 2 箇所

`from_whitespace` と `from_whitespace_interactive` がほぼ同一（差異は `with_interactive_stdin()` の有無のみ）。

## #[wasm_bindgen] エクスポート一覧 (22 個)

| JS 名 | 種別 | クラスタ |
|--------|------|----------|
| `run` | function | API |
| `compile` | function | API |
| `parse` | function | API |
| `compile_to_whitespace_string` | function | Helper |
| `compile_to_mnemonic_string` | function | Helper |
| `getOptions` | function | Helper |
| `WasmWhitespaceVM` constructor | method | VM |
| `fromWhitespace` | static method | VM |
| `fromWhitespaceInteractive` | static method | VM |
| `provideStdin` / `closeStdin` | method | VM |
| `step` / `pc` / `total_steps` / `is_complete` | method | VM |
| `get_stack` / `get_heap` / `call_stack_depth` | method | VM |
| `flush_stdout` / `get_traced` | method | VM |
| `current_instruction` / `disassemble` | method | VM |

## 分割方針

### ファイル構成案

```
src/wasm_api/
├── mod.rs              # モジュール公開・共通 imports
├── types.rs            # TS_TYPES, extern "C" 型, Serde 結果構造体, WasmError
├── pipeline.rs         # 共通パイプライン + パラメータパーサ + エラー変換
├── api.rs              # run, compile, parse, compile_to_*_string, get_options
└── whitespace_vm.rs    # SharedWriter, WasmWhitespaceVM + 全メソッド
```

### 各ファイルの詳細

#### types.rs (~150 行)

現在の L22–145 を集約:
- `TS_TYPES` (TypeScript カスタムセクション)
- `extern "C"` の 10 型: `JsRunResult`, `JsCompileResult`, `JsParseResult` 等
- Serde 結果構造体: `RunResultOk`, `ResultErr`, `CompileResultOk`, `WasmError`
- `ResultErr` のヘルパーコンストラクタ追加:

```rust
impl ResultErr {
    /// 単一エラーメッセージから ResultErr を構築
    pub fn single_error(message: String) -> Self {
        Self {
            success: false,
            errors: vec![WasmError {
                message,
                line: None,
                column: None,
            }],
        }
    }
}
```

これにより 7 箇所の手動構築を `ResultErr::single_error(msg)` に統一。

#### pipeline.rs (~130 行)

現在の L147–273 + 新規共通パイプライン:

```rust
use crate::{parse_to_tokens, parse_to_tree, syntactic_analyze, optimize, Scope};
use super::types::*;

/// JS パラメータからの std-ext パース
pub(super) fn parse_std_extensions(extensions: JsStdExtensionArray) -> Result<(bool, bool), ResultErr>

/// JS パラメータからの最適化パスパース
pub(super) fn parse_opt_passes(opt_passes: JsOptPassArray) -> Result<OptimizationOptions, ResultErr>

/// CodeParseError[] を ResultErr に変換
pub(super) fn convert_errors(errors: &[CodeParseError], text_code: &TextCode) -> ResultErr

/// 共通コンパイルパイプライン（4箇所の重複を解消）
pub(super) fn analyze_source(source: &str) -> Result<(Scope, TextCode), ResultErr> {
    let text_code = TextCode::new(source);
    let source_string = source.to_string();
    let tokens = parse_to_tokens(&source_string)
        .map_err(|e| convert_errors(&e, &text_code))?;
    let tree = parse_to_tree(&tokens)
        .map_err(|e| convert_errors(&e, &text_code))?;
    let scope = syntactic_analyze(&tree)
        .map_err(|e| convert_errors(&e, &text_code))?;
    Ok((scope, text_code))
}

/// 共通: パイプライン + 最適化適用
pub(super) fn analyze_and_optimize(
    source: &str,
    opt_passes: JsOptPassArray,
) -> Result<(Scope, TextCode, OptimizationOptions), ResultErr> {
    let (mut scope, text_code) = analyze_source(source)?;
    let opt_options = parse_opt_passes(opt_passes)?;
    if opt_options.any_enabled() {
        optimize(&mut scope, &opt_options);
    }
    Ok((scope, text_code, opt_options))
}
```

#### api.rs (~230 行)

現在の L275–510, L803–834。共通パイプラインを使用して大幅に簡略化:

```rust
#[wasm_bindgen]
pub fn run(source: &str, opt_passes: JsOptPassArray) -> JsRunResult {
    let (scope, text_code, _) = match pipeline::analyze_and_optimize(source, opt_passes) {
        Ok(v) => v,
        Err(e) => return serde_wasm_bindgen::to_value(&e).unwrap().into(),
    };
    // ... interpret + 結果構築
}

#[wasm_bindgen]
pub fn compile(source: &str, target: &str, std: &str, ...) -> JsCompileResult {
    let (scope, text_code, opt_options) = match pipeline::analyze_and_optimize(source, opt_passes) {
        Ok(v) => v,
        Err(e) => return serde_wasm_bindgen::to_value(&e).unwrap().into(),
    };
    // ... compile_to_whitespace + 結果構築
}

#[wasm_bindgen]
pub fn parse(source: &str) -> JsParseResult {
    let (scope, text_code) = match pipeline::analyze_source(source) {
        Ok(v) => v,
        Err(e) => return serde_wasm_bindgen::to_value(&e).unwrap().into(),
    };
    // ... 結果構築
}
```

#### whitespace_vm.rs (~300 行)

現在の L514–800。`WasmWhitespaceVM` と全 16 メソッド。
`SharedWriter` もここに配置（lib.rs 側の `SharedWriter` との重複解消は別タスク）。

`new()` コンストラクタも `pipeline::analyze_source` を使用:

```rust
#[wasm_bindgen(constructor)]
pub fn new(source: &str, std_ext: JsStdExtensionArray) -> Result<WasmWhitespaceVM, JsValue> {
    let (scope, text_code) = pipeline::analyze_source(source)
        .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;
    let (debug_ext, alloc_ext) = pipeline::parse_std_extensions(std_ext)
        .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;
    // ... compile + VM 構築
}
```

`from_whitespace` と `from_whitespace_interactive` の重複もヘルパーで解消:

```rust
fn create_from_ws_source(
    ws_source: &str,
    interactive: bool,
) -> Result<WasmWhitespaceVM, JsValue> {
    let vm = WhitespaceVM::from_source(ws_source)
        .map_err(|e| /* ResultErr 構築 */)?;
    let vm = if interactive {
        vm.with_interactive_stdin()
    } else {
        vm
    };
    // ... SharedWriter 設定
}
```

## 行数変化予測

| ファイル | 現在 | 分割後 | 備考 |
|----------|------|--------|------|
| wasm_api.rs | 834 | — | 削除 |
| wasm_api/mod.rs | — | ~20 | モジュール宣言のみ |
| wasm_api/types.rs | — | ~150 | 型定義集約 |
| wasm_api/pipeline.rs | — | ~130 | 共通パイプライン |
| wasm_api/api.rs | — | ~200 | API (重複解消で短縮) |
| wasm_api/whitespace_vm.rs | — | ~280 | VM ラッパー (重複解消で短縮) |
| **合計** | 834 | ~780 | 約 50 行削減 + 可読性向上 |

## wasm_bindgen の制約

- `#[wasm_bindgen]` は各ファイルで直接付与可能（モジュール分割に制約なし）
- `extern "C"` ブロック内の型はモジュールを跨いで使用可能
- `pub struct` + `#[wasm_bindgen]` のメソッドは同一 `impl` ブロック内に配置する必要がある → `whitespace_vm.rs` に全メソッドを集約するのは自然

## テストへの影響

- `wasm_api.rs` にユニットテストは存在しない
- WASM API のテストは JS 側（pkg/ ディレクトリ）で実施されるため、内部分割の影響なし

## リスク

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| `#[wasm_bindgen]` の型解決問題 | 低 | `wasm-pack build` で検証 |
| `cfg(feature = "wasm")` の伝播 | 低 | `mod.rs` で `#[cfg(feature = "wasm")]` を制御 |
| JS 側の API 互換性 | なし | エクスポート名は不変 |
