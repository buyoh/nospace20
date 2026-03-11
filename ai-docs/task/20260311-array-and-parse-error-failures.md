# 配列系テストと parse_error テスト失敗の修正設計

## 概要

以下 8 件の失敗テストについて、再現結果に基づいて原因を分離し、実装方針と検証計画を整理する。

- test_array_basic
- test_array_basic_opt_all
- test_array_basic_vm
- test_array_basic_ws_self
- test_array_basic_ws_self_opt_all
- test_array_basic_ws_self_randomize
- test_array_basic_ws_self_strict
- test_parse_error_struct_invalid_name_001

## 観測結果

### 1. 配列系 7 件

- すべて semantic analyzer で同一エラーになっている。
- エラーメッセージ: `semantic error: cannot use non-int expression as a value`
- 発生箇所: `src/semantic_analyzer/expression.rs` の `require_int_type`

補足:

- `resources/tests/passes/array-basic.ns` には `arr3` (配列変数) を値として使う記述がある。
- 現在は配列変数式の推論型が `ValueType::Array` となり、`require_int_type` が reject している。

### 2. parse_error 1 件

- `test_parse_error_struct_invalid_name_001` は `tests/code_test/error_base.rs` で失敗。
- エラー: `Unknown phase: parse`
- `resources/tests/fails/syntax/struct-invalid-name.check.json` の `phase` が `"parse"` で、テストハーネスは `"tokenize"` / `"tree"` しか受け付けていない。

## 原因分析

### A. 配列値利用の型判定が厳しすぎる

- 配列変数を値として使う既存テストケースに対し、`require_int_type` が `ValueType::Int` 以外を一律拒否している。
- この判定により、配列変数を伴う既存動作（特に `array-basic`）が回帰している。

### B. parse phase 名の後方互換不足

- `parse_error` の phase は実態として「字句解析以降の構文解析」扱い。
- 一部 fixture が `phase: parse` を使用しているが、ハーネス実装は `tree` のみ対応。

## 修正方針

## 方針1: 配列値利用を既存挙動に合わせる

- `src/semantic_analyzer/expression.rs` の `require_int_type` を調整し、`ValueType::Array(_, _)` を許容する。
- `ValueType::Void` は従来通り明示的に拒否する。
- エラーメッセージ文言は既存利用箇所との整合を崩さない。

期待効果:

- `array-basic` 系 7 テストが semantic エラーを回避し、既存の実行系テストまで進む。

## 方針2: parse phase エイリアスを追加

- `tests/code_test/error_base.rs` の `test_syntax_error_base` で `phase: parse` を `tree` 相当として扱う。
- 既存 `tokenize` / `tree` の挙動は維持。

期待効果:

- `test_parse_error_struct_invalid_name_001` がハーネス段階で失敗しなくなる。

## 影響範囲

- `src/semantic_analyzer/expression.rs`
- `tests/code_test/error_base.rs`

## 検証計画

1. ピンポイント再実行
   - `cargo test test_array_basic -- --nocapture`
   - `cargo test test_parse_error_struct_invalid_name_001 -- --nocapture`
2. 関連スイート確認
   - `cargo test --test code_test test_array_ -- --nocapture`
   - `cargo test --test code_test test_parse_error_ -- --nocapture`
3. 回帰確認（必要最小限）
   - `cargo test --test code_test -- --nocapture`

## リスク

- 配列型を広く許可しすぎると、本来拒否すべき文脈まで通る可能性がある。
- そのため今回の変更は `require_int_type` のみを最小変更し、追加で既存 parse_error 系を再実行して副作用を確認する。

## 進捗

### 2026-03-11

- 失敗 8 テストを再現。
- 配列系と parse phase 系に原因を分離。
- 修正方針と検証手順を確定。
