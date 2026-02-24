# ビルド時 warning の調査・修正計画

## 概要

`cargo build` および `cargo test --no-run` で発生する warning を調査し、修正方針をまとめる。

調査日: 2026-02-25

## 現在の warning 一覧

`cargo build` で 7 件、`cargo test --no-run` でさらに 7 件（テストコード由来）の warning が発生している。

---

## 1. ライブラリ・バイナリの warning（7 件）

### W1. `build.rs` — `TestCase.std_ext` フィールド未読

- **場所**: [build.rs](../../build.rs#L23)
- **メッセージ**: `field 'std_ext' is never read`
- **原因**: `TestCase` 構造体の `std_ext` フィールドは YAML のデシリアライズで読み込まれるが、Rust コード内で参照されていない。`exclude_std_ext` は使用されているが、`std_ext` は未使用。
- **対処**: YAML スキーマとの互換性のために残す場合は `#[allow(dead_code)]` を付与。不要であれば削除。
- **推奨**: `#[allow(dead_code)]` をフィールドに付与（YAML に `std_ext` キーを持つテストケースがある可能性があるため）

### W2. `src/compiler_ws/statement.rs:156` — 未使用変数 `func_name`

- **場所**: [src/compiler_ws/statement.rs](../../src/compiler_ws/statement.rs#L156)
- **メッセージ**: `unused variable: 'func_name'`
- **原因**: `generate_function_definition` 関数の引数 `func_name` が関数本体で使用されていない。
- **対処**: `_func_name` にリネーム、または将来デバッグラベル生成等で使用するなら `#[allow(unused_variables)]` を付与。
- **推奨**: `_func_name` にリネーム（最もシンプル）

### W3. `src/compiler_ws/mod.rs:39` — `CompileError::UndefinedFunction` 未構築

- **場所**: [src/compiler_ws/mod.rs](../../src/compiler_ws/mod.rs#L39)
- **メッセージ**: `variant 'UndefinedFunction' is never constructed`
- **原因**: 以前は `expression.rs` で使用されていたが、意味解析（semantic_analyzer）で事前にキャッチされるようになり不要になった。`UndefinedVariable` には既に `#[allow(dead_code)]` が付いているが、`UndefinedFunction` には付いていない。
- **対処**: `#[allow(dead_code)]` を付与、または削除。
- **推奨**: `#[allow(dead_code)]` を付与（`UndefinedVariable` と同様、防御的エラーバリアントとして残す）
- **関連**: [done-task/builtin-function-indexing.md](../done-task/builtin-function-indexing.md) で「互換性のため残す」と判断済み

### W4. `src/compiler_ws/mod.rs:75` — 関数 `compile` 未使用

- **場所**: [src/compiler_ws/mod.rs](../../src/compiler_ws/mod.rs#L75)
- **メッセージ**: `function 'compile' is never used`
- **原因**: `compile_with_options` が導入された後、オプションなしの `compile` は呼び出し元がなくなった。`lib.rs` や `bin/nospace20.rs` は `compile_with_options` を直接使用している。
- **対処**: 削除、または `#[allow(dead_code)]` を付与。
- **推奨**: 削除（`compile_with_options(scope, false)` のラッパーでしかなく、外部から使用されていない）

### W5. `src/compiler_ws/context.rs:94` — `CodeGenContext::new` 未使用

- **場所**: [src/compiler_ws/context.rs](../../src/compiler_ws/context.rs#L94)
- **メッセージ**: `associated function 'new' is never used`
- **原因**: W4 と同様、`new_with_options` が導入され `new` は呼び出されなくなった。`compile` 関数が削除されれば、さらに不要になる。
- **対処**: 削除、または `#[allow(dead_code)]` を付与。
- **推奨**: 削除（`new_with_options(scope, false)` のラッパーでしかない）

### W6. `src/semantic_analyzer/scope.rs:13` — `VariableIndex` のフィールド `0` 未読

- **場所**: [src/semantic_analyzer/scope.rs](../../src/semantic_analyzer/scope.rs#L13)
- **メッセージ**: `field '0' is never read`
- **原因**: `VariableIndex(pub usize)` は newtype として定義されているが、内部値 `.0` がどこからも読まれていない。`Identifier::Variable(VariableIndex)` のパターンマッチで存在確認には使われているが、値自体は取り出されていない。
- **対処**: 内部値を `_` にするか、`#[allow(dead_code)]` を付与。
- **推奨**: `#[allow(dead_code)]` を付与（将来的に変数インデックスとして活用する設計意図がある）
- **関連**: [done-task/identifier-management-improvement-completed.md](../done-task/identifier-management-improvement-completed.md) で既知

### W7. `src/bin/nospace20.rs:5-6` — 未使用 import

- **場所**: [src/bin/nospace20.rs](../../src/bin/nospace20.rs#L5)
- **メッセージ**: `unused imports: 'compile_to_whitespace_debug' and 'compile_to_whitespace'`
- **原因**: バイナリは `compile_to_whitespace_with_options` および `compile_to_whitespace_debug_with_options` のみ使用しており、オプションなし版は不要。
- **対処**: import から削除。
- **推奨**: import から削除（即座に修正可能）

---

## 2. テストコードの warning（7 件）

### W8. `src/compiler_ws/program.rs:97` — テストでの未使用 import `LabelId`

- **場所**: [src/compiler_ws/program.rs](../../src/compiler_ws/program.rs#L97)
- **メッセージ**: `unused import: 'LabelId'`
- **原因**: `#[cfg(test)]` モジュール内で import されているが、テストコードで使用されていない。
- **対処**: import から削除。
- **推奨**: import から削除

### W9-W12. `tests/common/mod.rs` — 4 つの未使用関数

- **場所**: [tests/common/mod.rs](../../tests/common/mod.rs)
- **関数**: `find_wsc` (L12), `which_wsc` (L32), `wsc_available` (L62), `run_whitespace` (L67)
- **メッセージ**: `function 'find_wsc' is never used` 等
- **原因**: これらの関数は `compile_test.rs` では使用されているが、`code_test.rs` 等の他のテストクレートでは使用されていないため、各テストバイナリのコンパイル時に警告が出る。Rust の統合テストは各ファイルが独立したクレートとしてコンパイルされるため、`common/mod.rs` はインクルードしたテストクレートごとに評価される。
- **対処**: `#[allow(dead_code)]` をモジュール先頭に付与。
- **推奨**: `#![allow(dead_code)]` を `tests/common/mod.rs` の先頭に付与

### W13-W14. `tests/code_test.rs` — 2 つの未使用関数

- **場所**: [tests/code_test.rs](../../tests/code_test.rs#L553) (L553, L616)
- **関数**: `test_whitespace_self_base`, `test_whitespace_self_io_base`
- **メッセージ**: `function 'test_whitespace_self_base' is never used` 等
- **原因**: これらの関数は `test_whitespace_self_base_debug` 経由でのみ呼び出されるが、build.rs が生成するテストコードからは `_debug`, `_strict`, `_randomize` バリアントが直接呼び出される。`test_whitespace_self_base` 自体は `_debug` のラッパーだが、生成コードからは直接呼ばれないため warning が出る。
- **対処**: `#[allow(dead_code)]` を付与。
- **推奨**: `#[allow(dead_code)]` を付与（自動生成コードからの間接使用のため）

---

## 修正方針まとめ

| # | 対処 | 難易度 |
|---|------|--------|
| W1 | `#[allow(dead_code)]` 付与 | 低 |
| W2 | `_func_name` にリネーム | 低 |
| W3 | `#[allow(dead_code)]` 付与 | 低 |
| W4 | `compile` 関数を削除 | 低 |
| W5 | `CodeGenContext::new` を削除 | 低 |
| W6 | `#[allow(dead_code)]` 付与 | 低 |
| W7 | 未使用 import 削除 | 低 |
| W8 | 未使用 import 削除 | 低 |
| W9-W12 | `#![allow(dead_code)]` 付与 | 低 |
| W13-W14 | `#[allow(dead_code)]` 付与 | 低 |

全て低難易度の修正で、機能への影響はない。

## 関連ドキュメント

- [done-task/unused-code-cleanup.md](../done-task/unused-code-cleanup.md) - 以前の未使用コード整理（2026-02-11 調査、多くは解消済み）
- [done-task/builtin-function-indexing.md](../done-task/builtin-function-indexing.md) - `UndefinedFunction` を残す判断
- [done-task/identifier-management-improvement-completed.md](../done-task/identifier-management-improvement-completed.md) - `VariableIndex` の dead_code 警告について
