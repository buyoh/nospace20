# deprecated 関数の移行・削除

日付: 2026-03-01
完了日: 2026-03-01

## ステータス: 完了

全 Step 完了。`cargo test` で全テスト合格（1645 passed, 0 failed）。

## 概要

`lib.rs` に残存する 7 つの deprecated 関数の利用箇所を新 API に移行し、deprecated 関数を削除する。

## 対象 deprecated 関数

| 関数 | 行 | 移行先 |
|------|-----|--------|
| `syntactic_analyze` | L52 | `semantic_analyze` |
| `compile_to_whitespace` | L251 | `compile_to_ws` + `WsCompileOptions` |
| `compile_to_whitespace_with_options` | L198 | `compile_to_ws` + `WsCompileOptions` |
| `compile_to_whitespace_with_opt` | L213 | `compile_to_ws` + `WsCompileOptions` |
| `compile_to_whitespace_debug` | L258 | `compile_to_ws` + `WsCompileOptions { output_format: Mnemonic }` |
| `compile_to_whitespace_debug_with_options` | L226 | `compile_to_ws` + `WsCompileOptions` |
| `compile_to_whitespace_debug_with_opt` | L238 | `compile_to_ws` + `WsCompileOptions` |

## 利用箇所一覧

### `syntactic_analyze` の利用箇所

#### `src/optimizer/tests.rs`（50箇所以上）

パターン: `crate::syntactic_analyze(&s).unwrap()`

移行: `crate::semantic_analyze(&s).unwrap()` に一括置換。

#### `tests/code_test/interpreter_base.rs`

```rust
// Before (L5)
use nospace20::syntactic_analyze;
// After
use nospace20::semantic_analyze;
```

L18, 63, 114, 149, 193 の呼出しも置換。

#### `tests/code_test/whitespace_base.rs`

L3 の import + L24, 72 の呼出し。

#### `tests/code_test/whitespace_self_base.rs`

L5 の import + L16, 67, 169, 214, 284, 385 の呼出し。

#### `tests/code_test/error_base.rs`

L5 の import + L69, 139 の呼出し。

#### `tests/compile_test.rs`

L10 の import + L24, 51, 79 の呼出し。

#### `tests/ignore_debug_test/helpers.rs`

L8 の import + L49 の呼出し。

### `compile_to_whitespace` の利用箇所

| ファイル | 行 | 移行方法 |
|----------|-----|----------|
| `tests/code_test/whitespace_base.rs` | L3, 26, 74 | `compile_to_ws` + `WsCompileOptions::default()` |
| `tests/code_test/error_base.rs` | L4, 93 | 同上 |
| `tests/compile_test.rs` | L9, 53, 81 | 同上 |

### `compile_to_whitespace_with_options` の利用箇所

| ファイル | 行 | 移行方法 |
|----------|-----|----------|
| `tests/code_test/whitespace_self_base.rs` | L5, 18, 69, 170, 215, 285, 386 | `compile_to_ws` + `WsCompileOptions { debug_ext, alloc_ext, .. }` |

### `compile_to_whitespace_debug` の利用箇所

| ファイル | 行 | 移行方法 |
|----------|-----|----------|
| `tests/compile_test.rs` | L9, 26 | `compile_to_ws` + `WsCompileOptions { output_format: Mnemonic, .. }` |

### 外部使用なしの関数

以下は外部からの直接呼出しがなく、定義のみ：

- `compile_to_whitespace_with_opt`
- `compile_to_whitespace_debug_with_options`（`compile_to_whitespace_debug` からの内部呼出しのみ）
- `compile_to_whitespace_debug_with_opt`

## 作業ステップ

### Step 1: `syntactic_analyze` → `semantic_analyze` の一括置換

1. `src/optimizer/tests.rs` 内の `crate::syntactic_analyze` → `crate::semantic_analyze` を一括置換
2. `tests/` 配下の各ファイルで import と呼出しを置換
3. `cargo test` で全テスト通過を確認

### Step 2: `compile_to_whitespace*` → `compile_to_ws` の移行

各テストファイルで import 変更 + `WsCompileOptions` 構造体を使用した呼出しに変更。

```rust
// Before
use nospace20::compile_to_whitespace;
let ws = compile_to_whitespace(&scope).unwrap();

// After
use nospace20::{compile_to_ws, WsCompileOptions};
let ws = compile_to_ws(&scope, &WsCompileOptions::default()).unwrap();
```

```rust
// Before (with options)
use nospace20::compile_to_whitespace_with_options;
let ws = compile_to_whitespace_with_options(&scope, debug_ext, alloc_ext).unwrap();

// After
use nospace20::{compile_to_ws, WsCompileOptions};
let ws = compile_to_ws(&scope, &WsCompileOptions {
    debug_ext,
    alloc_ext,
    ..Default::default()
}).unwrap();
```

```rust
// Before (debug mnemonic)
use nospace20::compile_to_whitespace_debug;
let mnemonic = compile_to_whitespace_debug(&scope).unwrap();

// After
use nospace20::{compile_to_ws, WsCompileOptions, WsOutputFormat};
let mnemonic = compile_to_ws(&scope, &WsCompileOptions {
    output_format: WsOutputFormat::Mnemonic,
    ..Default::default()
}).unwrap();
```

### Step 3: deprecated 関数の削除

`src/lib.rs` から 7 つの deprecated 関数を削除（約 70 行の削減）。

### Step 4: テスト確認

```bash
cargo test
cargo test --features wasm  # WASM ビルドに影響しないことを確認
```

## 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/lib.rs` | deprecated 関数 7 つを削除 |
| `src/optimizer/tests.rs` | `syntactic_analyze` → `semantic_analyze`（50箇所以上） |
| `tests/code_test/interpreter_base.rs` | import + 呼出し置換 |
| `tests/code_test/whitespace_base.rs` | import + 呼出し置換 |
| `tests/code_test/whitespace_self_base.rs` | import + 呼出し置換 |
| `tests/code_test/error_base.rs` | import + 呼出し置換 |
| `tests/compile_test.rs` | import + 呼出し置換 |
| `tests/ignore_debug_test/helpers.rs` | import + 呼出し置換 |

## 作業見積もり

小〜中。単純な検索置換が大半だが、`compile_to_whitespace_with_options` の移行は `WsCompileOptions` 構造体の組み立てが必要。
