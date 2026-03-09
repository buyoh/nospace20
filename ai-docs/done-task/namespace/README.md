# 名前空間 (namespace) 設計

nospace 言語に名前空間機能を追加する設計ドキュメント。

## 概要

名前空間は識別子の名前衝突を回避するための名前修飾機構である。
**スコープではない**ため、変数の確保・解放タイミングは名前空間の外側の `{}` に依存する。

## ドキュメント

1. [spec.md](spec.md) - 言語仕様（構文・セマンティクス・制約）
2. [implementation.md](implementation.md) - 実装設計（モジュールごとの変更方針）
3. [test-plan.md](test-plan.md) - テスト計画

## 設計方針

- 名前空間は**コンパイル時の名前修飾**であり、ランタイムコストを導入しない
- 既存の `Scope` 構造体はスコープ管理に使用されるが、名前空間は独立した名前修飾の層として実装する
- Whitespace コンパイラ・インタプリタへの影響を最小限にするため、意味解析の段階で名前をフラット化（マングリング）する

## 実装進捗

- ✅ Step 1: Token Parser — `Namespace` キーワード、`Dollar` トークン追加完了
- ✅ Step 2: Tree Parser — `NamespaceDeclaration` のパース、修飾識別子のパース完了
- ✅ Step 3: Semantic Analyzer — マングリング、名前解決拡張完了
- ✅ Large テスト追加 (passes/namespace/ に 7 件、fails/compile/ に 2 件)
- ✅ 全テスト合格（リグレッションなし）
