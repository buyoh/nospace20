# コード全体の設計レビュー（残タスク）

日付: 2026-03-01

## 概要

プロジェクト全体のコード設計をレビューし、改善点を洗い出した。
完了済みタスクは [done-task/code-design-review/](../../done-task/code-design-review/) に移動済み。

## 残タスク

### カテゴリ別レビュー結果

| ファイル | 内容 | 状態 |
|----------|------|------|
| ~~01-error-handling.md~~ | エラー型の統一・エラーハンドリング改善 | **完了** → [done-task](../../done-task/code-design-review/01-error-handling.md) |
| [02-module-splitting.md](02-module-splitting.md) | 巨大モジュールの分割・責務分離（概要） | 未着手 |
| ~~03-code-duplication.md~~ | コード重複の解消 | **完了** → [done-task](../../done-task/code-design-review/03-code-duplication.md) |
| ~~04-api-design.md~~ | 公開 API の設計改善 | **完了** → [done-task](../../done-task/code-design-review/04-api-design.md) |
| ~~05-rust-idioms.md~~ | Rust イディオム・型安全性の改善 | **完了** → [done-task](../../done-task/code-design-review/05-rust-idioms.md) |
| ~~06-dependency-structure.md~~ | モジュール間依存構造の改善 | **完了** → [done-task](../../done-task/code-design-review/06-dependency-structure.md) |
10. **命名の改善** — `syntactic_analyze` → `semantic_analyze`、`logger` → `source_map` ([詳細](05-rust-idioms.md#命名))
