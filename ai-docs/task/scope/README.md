# スコープ機能の実装

このディレクトリには、スコープ機能の実装に関する設計・検討ドキュメントが含まれます。

## 現在のタスク: Phase 5

- [phase5-nested-functions.md](phase5-nested-functions.md) - フェーズ5: ネスト関数のスコープ制御

### 完了したフェーズ

- [../../done-task/scope-phase1-block-scope.md](../../done-task/scope-phase1-block-scope.md) - フェーズ1: ブロックスコープ変数の最小実装 ✅ 完了
- [../../done-task/scope-phase2-identifier-resolution.md](../../done-task/scope-phase2-identifier-resolution.md) - フェーズ2: 識別子の事前解決 ✅ 完了
- [../../done-task/scope-phase3-global-variables.md](../../done-task/scope-phase3-global-variables.md) - フェーズ3: グローバル変数 ✅ 完了
- [../../done-task/scope-phase4-static-variables.md](../../done-task/scope-phase4-static-variables.md) - フェーズ4: static 変数 ✅ 完了
- [../../done-task/scope-phase4-design.md](../../done-task/scope-phase4-design.md) - フェーズ4: 設計ドキュメント
- [../../done-task/scope-analysis.md](../../done-task/scope-analysis.md) - Phase 1-4 の全体分析
- [../../done-task/scope-test-coverage-review.md](../../done-task/scope-test-coverage-review.md) - テストカバレッジレビュー
- [../../done-task/scope-previous-implementation.md](../../done-task/scope-previous-implementation.md) - 過去の実装コミット（7a83612）の分析 ✅ アーカイブ

## 関連仕様

- [spec.md](../../../spec.md) セクション 7 - スコープの言語仕様
- [unimplemented-features.md](../unimplemented-features.md) - 未実装機能一覧

## 関連コード

- `src/semantic_analyzer/mod.rs` - 意味解析器（スコープ構造構築、識別子解決）
- `src/interpreter/mod.rs` - インタプリタ（実行時変数管理）

## 関連テスト

- `resources/tests/passes/scope/` - スコープ関連テストケース
