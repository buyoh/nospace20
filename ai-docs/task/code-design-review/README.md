# コード全体の設計レビュー

日付: 2026-03-01

## 概要

プロジェクト全体のコード設計をレビューし、改善点を洗い出した。
改善項目を優先度・カテゴリごとに分類し、各ドキュメントに詳細を記載。

## ドキュメント構成

### カテゴリ別レビュー結果

| ファイル | 内容 |
|----------|------|
| [01-error-handling.md](01-error-handling.md) | エラー型の統一・エラーハンドリング改善 |
| [02-module-splitting.md](02-module-splitting.md) | 巨大モジュールの分割・責務分離（概要） |
| [03-code-duplication.md](03-code-duplication.md) | コード重複の解消 |
| [04-api-design.md](04-api-design.md) | 公開 API の設計改善 |
| [05-rust-idioms.md](05-rust-idioms.md) | Rust イディオム・型安全性の改善 |
| [06-dependency-structure.md](06-dependency-structure.md) | モジュール間依存構造の改善 |

### モジュール分割の詳細設計

| ファイル | 対象 | 現在行数 | 改善方針 |
|----------|------|---------|----------|
| [split-semantic-analyzer.md](split-semantic-analyzer.md) | semantic_analyzer/mod.rs | 1801行 | 3 Phase 分割: constexpr/alias/template 等を分離 |
| [split-wasm-api.md](split-wasm-api.md) | wasm_api.rs | 834行 | 4 ファイル分割 + パイプライン重複解消 |
| [split-compiler-ws-expression.md](split-compiler-ws-expression.md) | compiler_ws/expression.rs | 1020行 | コード重複解消 (void 統合/比較データ駆動化) |
| [split-alloc-runtime.md](split-alloc-runtime.md) | compiler_ws/alloc_runtime.rs | 1713行 | 3 ファイル分割 (trait/bump/fsba) |

## 優先度サマリ

### High（安全性・保守性に直結）

1. **unsafe キャストの除去** — `whitespace/interpreter.rs` の `get_stdout_string()` ([詳細](06-dependency-structure.md#unsafe-キャストの除去))
2. **ライブラリからの `eprintln!` / `process::exit` 除去** — テスタビリティ低下 ([詳細](04-api-design.md#副作用の除去))
3. **エラー型の統一** — `CodeParseError` / `CompileError` / `String` の混在 ([詳細](01-error-handling.md))

### Medium（設計品質・拡張性）

4. **`semantic_analyzer/mod.rs` の分割** — 1801 行の巨大モジュール ([詳細](02-module-splitting.md#semantic_analyzer))
5. **コード重複の解消** — SharedWriter, constexpr 評価, randomize_uninit 等 ([詳細](03-code-duplication.md))
6. **`whitespace` → `compiler_ws` の依存方向修正** — 共有型の独立モジュール化 ([詳細](06-dependency-structure.md#共有型の独立モジュール化))
7. **`lib.rs` の公開 API 整理** — compile_to_whitespace 系 6 関数の統合 ([詳細](04-api-design.md#compile-api-統合))

### Low（品質向上・イディオム改善）

8. **Cargo.toml 改善** — edition 2021 移行、assert_matches の dev-dependencies 化 ([詳細](05-rust-idioms.md#cargo-toml))
9. **型安全性の向上** — PrettyToken 構造体化、Statement バリアント構造化 ([詳細](05-rust-idioms.md#型設計))
10. **命名の改善** — `syntactic_analyze` → `semantic_analyze`、`logger` → `source_map` ([詳細](05-rust-idioms.md#命名))
