# インラインテストの分離

## 概要

`src/` 内の多くの `.rs` ファイルで、実装コードとユニットテスト (`#[cfg(test)] mod tests`) が同一ファイルに記述されている。
テストコードを別ファイルに分離し、可読性と保守性を向上させる。

## 背景

一部のモジュールでは既にテストが分離されている（例: `optimizer/mod.rs` → `optimizer/tests.rs`）。
しかし、多くのファイルでは数百行規模のテストコードがインラインで含まれており、ファイルが肥大化している。

### 既に分離済みのモジュール

| モジュール | テスト参照 |
|-----------|-----------|
| `optimizer/mod.rs` | `mod tests;` (別ファイル) |
| `token_parser/mod.rs` | `mod test;` (別ファイル) |
| `tree_parser/expression/mod.rs` | `mod test;` (別ファイル) |
| `tree_parser/statement/mod.rs` | `mod test;` (別ファイル) |
| `semantic_analyzer/mod.rs` | `#[path = "tests.rs"] mod tests;` (別ファイル) |
| `logger/mod.rs` | `mod test;` (別ファイル) |

### 分離不要

| モジュール | 理由 |
|-----------|------|
| `lib.rs` | `#[cfg(test)]` は `extern crate` 宣言のみ、テストモジュールなし |

## 対象ファイル一覧

テスト行数（推定）の降順でソート。全ファイルが `#[cfg(test)] mod tests { ... }` パターン。

### 優先度 高（テスト行数 200 行以上）

| ファイル | 総行数 | テスト開始行 | テスト行数(推定) | テスト割合 |
|---------|-------|------------|----------------|----------|
| `interpreter/exec.rs` | 1205 | L705 | ~500 | 41% |
| `compiler_ws/alloc_runtime/fsba.rs` | 1100 | L709 | ~391 | 36% |
| `whitespace/interpreter.rs` | 1261 | L965 | ~296 | 23% |
| `compiler_ws/alloc_runtime/bump.rs` | 368 | L95 | ~274 | 74% |
| `interpreter/allocator.rs` | 644 | L379 | ~265 | 41% |
| `base/constexpr_eval.rs` | 599 | L337 | ~262 | 44% |

### 優先度 中（テスト行数 100〜200 行）

| ファイル | 総行数 | テスト開始行 | テスト行数(推定) | テスト割合 |
|---------|-------|------------|----------------|----------|
| `compiler_ws/label.rs` | 297 | L121 | ~176 | 59% |
| `compiler_ws/peephole.rs` | 349 | L215 | ~135 | 39% |
| `base/ws_types.rs` | 483 | L362 | ~122 | 25% |
| `whitespace/parser.rs` | 365 | L264 | ~102 | 28% |
| `optimizer/mod.rs` (確認: 既に分離済み) | — | — | — | — |

### 優先度 低（テスト行数 100 行未満）

| ファイル | 総行数 | テスト開始行 | テスト行数(推定) | テスト割合 |
|---------|-------|------------|----------------|----------|
| `base/error/mod.rs` | 203 | L123 | ~81 | 40% |
| `base/pure_eval.rs` | 134 | L55 | ~80 | 60% |
| `compiler_ws/alloc_runtime/mod.rs` | 226 | L157 | ~70 | 31% |
| `base/error/ws_error.rs` | 147 | L92 | ~56 | 38% |
| `algorithm/alloc_spec.rs` | 125 | L72 | ~54 | 43% |
| `compiler_ws/memory.rs` | 128 | L94 | ~35 | 27% |
| `base/error/compile_error.rs` | 87 | L66 | ~22 | 25% |
| `base/error/parse_error.rs` | 76 | L55 | ~22 | 29% |
| `base/error/interpret_error.rs` | 52 | L31 | ~22 | 42% |

## 分離方法

各ファイルに対して以下の手順を適用する。

### 手順

1. **テストファイル作成**: 同一ディレクトリに `tests.rs`（または同名モジュールの `test.rs`）を作成
2. **テストコード移動**: `#[cfg(test)] mod tests { ... }` の中身をテストファイルに移動
3. **モジュール宣言に置換**: 元ファイルの `#[cfg(test)] mod tests { ... }` を以下に置換:
   ```rust
   #[cfg(test)]
   mod tests;
   ```
4. **テストファイル先頭に `use super::*;` を記述**: 元ファイルのスコープにアクセスするため
5. **テスト実行**: `cargo test` で全テストが通ることを確認

### ファイル命名規則

既存パターンに従う:
- `mod.rs` のテスト → `tests.rs`（`semantic_analyzer` の例に従う）
- 単一ファイル（例: `interpreter.rs`）のテスト → 同ディレクトリに `tests.rs` は不可（他モジュールと衝突の可能性）。以下のいずれか:
  - ファイルをディレクトリモジュールに変換する（例: `interpreter.rs` → `interpreter/mod.rs` + `interpreter/tests.rs`）— **大規模な変更**
  - `#[path = "interpreter_tests.rs"]` アトリビュートを使用する — **最小変更**

本タスクでは `#[path]` アトリビュートを優先的に使用し、ディレクトリ変換は行わない。

#### 具体的な命名

| 元ファイル | テストファイル | モジュール宣言 |
|-----------|-------------|-------------|
| `interpreter/exec.rs` | `interpreter/exec_tests.rs` | `#[cfg(test)] #[path = "exec_tests.rs"] mod tests;` |
| `interpreter/allocator.rs` | `interpreter/allocator_tests.rs` | `#[cfg(test)] #[path = "allocator_tests.rs"] mod tests;` |
| `whitespace/interpreter.rs` | `whitespace/interpreter_tests.rs` | `#[cfg(test)] #[path = "interpreter_tests.rs"] mod tests;` |
| `whitespace/parser.rs` | `whitespace/parser_tests.rs` | `#[cfg(test)] #[path = "parser_tests.rs"] mod tests;` |
| `compiler_ws/label.rs` | `compiler_ws/label_tests.rs` | `#[cfg(test)] #[path = "label_tests.rs"] mod tests;` |
| `compiler_ws/peephole.rs` | `compiler_ws/peephole_tests.rs` | `#[cfg(test)] #[path = "peephole_tests.rs"] mod tests;` |
| `compiler_ws/memory.rs` | `compiler_ws/memory_tests.rs` | `#[cfg(test)] #[path = "memory_tests.rs"] mod tests;` |
| `compiler_ws/alloc_runtime/mod.rs` | `compiler_ws/alloc_runtime/tests.rs` | `#[cfg(test)] mod tests;` |
| `compiler_ws/alloc_runtime/bump.rs` | `compiler_ws/alloc_runtime/bump_tests.rs` | `#[cfg(test)] #[path = "bump_tests.rs"] mod tests;` |
| `compiler_ws/alloc_runtime/fsba.rs` | `compiler_ws/alloc_runtime/fsba_tests.rs` | `#[cfg(test)] #[path = "fsba_tests.rs"] mod tests;` |
| `base/constexpr_eval.rs` | `base/constexpr_eval_tests.rs` | `#[cfg(test)] #[path = "constexpr_eval_tests.rs"] mod tests;` |
| `base/ws_types.rs` | `base/ws_types_tests.rs` | `#[cfg(test)] #[path = "ws_types_tests.rs"] mod tests;` |
| `base/pure_eval.rs` | `base/pure_eval_tests.rs` | `#[cfg(test)] #[path = "pure_eval_tests.rs"] mod tests;` |
| `base/error/mod.rs` | `base/error/tests.rs` | `#[cfg(test)] mod tests;` |
| `base/error/ws_error.rs` | `base/error/ws_error_tests.rs` | `#[cfg(test)] #[path = "ws_error_tests.rs"] mod tests;` |
| `base/error/compile_error.rs` | `base/error/compile_error_tests.rs` | `#[cfg(test)] #[path = "compile_error_tests.rs"] mod tests;` |
| `base/error/parse_error.rs` | `base/error/parse_error_tests.rs` | `#[cfg(test)] #[path = "parse_error_tests.rs"] mod tests;` |
| `base/error/interpret_error.rs` | `base/error/interpret_error_tests.rs` | `#[cfg(test)] #[path = "interpret_error_tests.rs"] mod tests;` |
| `algorithm/alloc_spec.rs` | `algorithm/alloc_spec_tests.rs` | `#[cfg(test)] #[path = "alloc_spec_tests.rs"] mod tests;` |

### 注意事項

- `compiler_ws/label.rs` L91 の `#[cfg(test)]` はテストモジュールではなく、テスト専用メソッド (`has_function`) のアトリビュート。分離対象は L121 の `#[cfg(test)] mod tests` のみ。
- `semantic_analyzer/mod.rs` L42 の `#[cfg(test)]` はテストモジュールではなく、テスト時のみの `use` 文。分離対象外（既に L324 で `#[path = "tests.rs"] mod tests;` として分離済み）。
- `base/error/mod.rs` を分離する際、テストファイルの中で `use super::*;` では不足する場合がある。テスト内の `use` 文を確認して適切に移行すること。

## 作業計画

モジュール単位で段階的に実施する。各段階でテストを実行し、全テスト通過を確認する。

### Step 1: 優先度 高（テスト行数 200 行以上）

6 ファイルを分離:
- `interpreter/exec.rs`
- `compiler_ws/alloc_runtime/fsba.rs`
- `whitespace/interpreter.rs`
- `compiler_ws/alloc_runtime/bump.rs`
- `interpreter/allocator.rs`
- `base/constexpr_eval.rs`

### Step 2: 優先度 中（テスト行数 100〜200 行）

4 ファイルを分離:
- `compiler_ws/label.rs`
- `compiler_ws/peephole.rs`
- `base/ws_types.rs`
- `whitespace/parser.rs`

### Step 3: 優先度 低（テスト行数 100 行未満）

9 ファイルを分離:
- `base/error/mod.rs`
- `base/pure_eval.rs`
- `compiler_ws/alloc_runtime/mod.rs`
- `base/error/ws_error.rs`
- `algorithm/alloc_spec.rs`
- `compiler_ws/memory.rs`
- `base/error/compile_error.rs`
- `base/error/parse_error.rs`
- `base/error/interpret_error.rs`

## 目標

- [x] 対象ファイルの調査・一覧化
- [x] Step 1 実施（6 ファイル）
- [x] Step 2 実施（4 ファイル）
- [x] Step 3 実施（9 ファイル）
- [x] 全テスト通過確認

## 完了

全 19 ファイルの分離が完了。全テスト (366 + 1246 + その他) 通過確認済み。

### 実装上の注意点

`compiler_ws/alloc_runtime/mod.rs` の `pub(super) mod test_helpers` は `mod tests` ではなく、`test_helpers` という名前を維持した（`bump.rs`・`fsba.rs` のテストがこのモジュールを参照しているため）。
テストファイルは `test_helpers.rs`、宣言は `#[cfg(test)] pub(super) mod test_helpers;` として分離した。

## 関連ドキュメント

- [code-design-review/02-module-splitting.md](../done-task/code-design-review/02-module-splitting.md) — モジュール分割の過去タスク（実装コードの分割。テスト分離とは異なる）
