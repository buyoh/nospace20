# スコープ機能の実装

このディレクトリには、スコープ機能の実装に関する設計・検討ドキュメントが含まれます。

## ドキュメント一覧

- [overview.md](overview.md) - 現状分析とスコープ実装の概要
- [phase1-block-scope.md](phase1-block-scope.md) - フェーズ1: ブロックスコープ変数の最小実装
- [previous-implementation.md](previous-implementation.md) - 過去の実装コミット（7a83612）の分析

## 関連仕様

- [spec.md](../../../spec.md) セクション 7 - スコープの言語仕様
- [unimplemented-features.md](../unimplemented-features.md) - 未実装機能一覧

## 関連コード

- `src/semantic_analyzer/mod.rs` - 意味解析器（スコープ構造構築）
- `src/interpreter/mod.rs` - インタプリタ（実行時変数管理）

## 関連テスト

- `resources/tests/passes/scope/` - スコープ関連テストケース
