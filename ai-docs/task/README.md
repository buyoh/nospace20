# Task

現在作業中のタスク・進捗を記録するディレクトリ。

## ドキュメント

- [unimplemented-features.md](unimplemented-features.md) - 未実装の機能・構文の一覧
- [code-structure-refactoring.md](code-structure-refactoring.md) - ソースコード構成のリファクタリング計画
- [unit-test-analysis.md](unit-test-analysis.md) - ユニットテスト分析レポート

## 現在のタスク

### アクティブ

- [migrate-legacy-tests.md](migrate-legacy-tests.md) - 旧テストの移行計画
  - ⚠️ I/O ビルトイン関数の実装が必要
  - 計画立案完了、実装待ち

- [test-error-handling.md](test-error-handling.md) - テストのエラーハンドリング強化
  - ✅ Phase 1-2完了
  - ⚠️ ブロックスコープ変数の実装を解決する必要あり

### 完了済み

- [test-categorization.md](test-categorization.md) - テストケースのカテゴリ分け

## タスクファイルの命名規則

- `YYYYMMDD-task-name.md` - 日付付きタスクファイル
- `ongoing-task-name.md` - 継続的なタスク

## タスクファイルのテンプレート

```markdown
# タスク名

## 概要

タスクの説明

## 目標

- [ ] 目標1
- [ ] 目標2

## 進捗

### YYYY-MM-DD

- 作業内容

## 関連ファイル

- path/to/file.rs

## メモ

追加情報
```

## 完了したタスク

| 日付 | タスク | 備考 |
|------|--------|------|
| 2026-01-29 | ai-docs 初期構築 | アーキテクチャ・仕様ドキュメント作成 |
