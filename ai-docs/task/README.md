# Task

現在作業中のタスク・進捗を記録するディレクトリ。

## ドキュメント

- [unimplemented-features.md](unimplemented-features.md) - 未実装の機能・構文の一覧（継続追跡）
- [unit-test-interpreter.md](unit-test-interpreter.md) - interpreter ユニットテスト追加タスク
- [integration-test-design.md](integration-test-design.md) - 結合テスト設計・計画
- [whitespace-integration-test.md](whitespace-integration-test.md) - Whitespace コンパイラ統合テスト設計
- [compiler/](compiler/) - nospace → Whitespace コンパイラ調査 (旧実装分析)
- [scope/](scope/) - スコープ機能の実装計画

## 現在のタスク

### アクティブ

（現在アクティブなタスクはありません）

### 完了済み (done-task/ に移動)

- [compile-test-refactoring.md](../done-task/compile-test-refactoring.md) - compile_test.rs のリファクタリング設計 (2026-02-07完了)
  - compile_test.rs を13件→1件に縮小 (92%削減)
  - 新規テストタイプ compile_error を追加
  - Whitespace テストに空白文字検証を追加
- [semantic-analyzer-error-handling.md](../done-task/semantic-analyzer-error-handling.md) - Semantic Analyzer エラーハンドリング改善 (2026-02-07完了)
  - panic! を Result 型に変更
  - 位置情報を付与してエラーメッセージを強化
  - Phase 1 (Result型への移行) と Phase 2 (位置情報の付与) を完了
- [multiple-io-test-cases.md](../done-task/multiple-io-test-cases.md) - 複数の入出力テストケース対応 (2026-02-07完了)
  - success_io テストで複数ケースをサポート
  - 後方互換性を維持しつつ、1テストで複数パターンをテスト可能に
  - 全70テストが成功
- [legacy-migration-phase3.md](../done-task/legacy-migration-phase3.md) - 旧テストの移行 Phase 3 (2026-02-07完了)
  - else:if構文のパーサー対応
  - legacy_009, 010の有効化
  - 新しいI/Oテストケースの作成
- [yaml-test-generation.md](../done-task/yaml-test-generation.md) - YAMLベースのテスト自動生成 (2026-02-04完了)
- [cli-compile-options.md](../done-task/cli-compile-options.md) - CLIコンパイルオプション設計・実装 (2026-02-04完了)
- [code-structure-refactoring.md](../done-task/code-structure-refactoring.md) - ソースコード構成のリファクタリング (2026-02-04完了)
- [test-categorization.md](../done-task/test-categorization.md) - テストケースのカテゴリ分け
- [implement-new-features.md](../done-task/implement-new-features.md) - 新機能実装 (文字リテラル、論理演算子、剰余演算子)
- [unit-test-analysis.md](../done-task/unit-test-analysis.md) - ユニットテスト分析レポート
- [unit-test-tree-parser.md](../done-task/unit-test-tree-parser.md) - tree_parser ユニットテスト追加タスク (2026-02-01完了)
- [unit-test-semantic-analyzer.md](../done-task/unit-test-semantic-analyzer.md) - semantic_analyzer ユニットテスト追加タスク (2026-02-01完了)
- [migrate-legacy-tests.md](../done-task/migrate-legacy-tests.md) - 旧テストの移行計画 (2026-02-01完了)
- [cli-improvements.md](../done-task/cli-improvements.md) - CLI改善（ファイル引数、デバッグフラグ、ヘルプ表示） (2026-02-01完了)
- [legacy-migration-phase2-report.md](../done-task/legacy-migration-phase2-report.md) - 旧テストの移行 Phase 2 完了報告 (2026-01-31完了)

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
