# スコープ機能の実装

このディレクトリには、スコープ機能の実装に関する設計・検討ドキュメントが含まれます。

## ドキュメント一覧

- [overview.md](overview.md) - 現状分析とスコープ実装の概要
- [phase4-static-variables.md](phase4-static-variables.md) - フェーズ4: static 変数 📋 設計完了
- [test-coverage-review.md](test-coverage-review.md) - スコープ機能のテストカバレッジレビュー

### 完了したフェーズ

- [../../done-task/scope-phase1-block-scope.md](../../done-task/scope-phase1-block-scope.md) - フェーズ1: ブロックスコープ変数の最小実装 ✅ 完了
- [../../done-task/scope-phase2-identifier-resolution.md](../../done-task/scope-phase2-identifier-resolution.md) - フェーズ2: 識別子の事前解決 ✅ 完了
- [../../done-task/scope-phase3-global-variables.md](../../done-task/scope-phase3-global-variables.md) - フェーズ3: グローバル変数 ✅ 完了
- [../../done-task/scope-previous-implementation.md](../../done-task/scope-previous-implementation.md) - 過去の実装コミット（7a83612）の分析 ✅ アーカイブ

## 関連仕様

- [spec.md](../../../spec.md) セクション 7 - スコープの言語仕様
- [unimplemented-features.md](../unimplemented-features.md) - 未実装機能一覧

## 関連コード

- `src/semantic_analyzer/mod.rs` - 意味解析器（スコープ構造構築、識別子解決）
- `src/interpreter/mod.rs` - インタプリタ（実行時変数管理）

## 関連テスト

- `resources/tests/passes/scope/` - スコープ関連テストケース
