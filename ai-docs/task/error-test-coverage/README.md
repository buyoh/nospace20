# エラーテストケース網羅性向上

## 概要

nospace コンパイラ・インタプリタが出力するエラーメッセージに対し、テストケースの網羅性を向上させる。
ソースコードから抽出した全エラーパスに対して、テストがあるかを精査し、不足分を追加する。

## 背景

[error-specification](../../done-task/error-specification/) で全エラーメッセージの調査・分類は完了済みだが、テストカバレッジが不十分。

### 現状

- **syntax_error テスト**: 8 ファイル（5 マニフェスト登録）
- **compile_error テスト**: 12 ファイル（12 マニフェスト登録）
- ソースコード上のエラーパス: 約 65 箇所（panic/unreachable 除く）

### 目標

ユーザーが遭遇する可能性のあるエラーパスのテストを追加し、`contains` による部分文字列チェックでエラーメッセージの内容を検証する。

## スコープ

### 対象

- **字句解析エラー** (token_parser) — `parse_error` / `phase: "tokenize"`
- **構文解析エラー** (tree_parser) — `parse_error` / `phase: "tree"`
- **意味解析エラー** (semantic_analyzer) — `compile_error`
- **コンパイルエラー** (compiler_ws) — `compile_error`

### 対象外

- **実行時エラー** (RuntimeError) — 専用のテストフレームワークが未整備（別タスクで検討）
- **Whitespace パーサーエラー** (ParseError) — nospace ユーザーが直接遭遇しない
- **panic!/unreachable!** — 内部整合性チェック、テスト不要
- **nospace インタプリタの panic** — 実行時エラーフレームワーク整備後に対応

## ドキュメント構成

- [README.md](README.md) — 本ファイル（全体概要）
- [coverage-matrix.md](coverage-matrix.md) — エラーパスとテストケースの網羅性マトリクス
- [test-plan.md](test-plan.md) — 追加するテストケースの詳細設計

## タスク

- [ ] **Phase 1**: 字句解析エラーのテスト追加（6 件）
- [ ] **Phase 2**: 構文解析エラーのテスト追加（7 件）
- [ ] **Phase 3**: 意味解析エラーのテスト追加（5 件）
- [ ] **Phase 4**: テスト結果の検証・マトリクス更新

## 関連ドキュメント

- [error-specification](../../done-task/error-specification/) - エラー仕様ドキュメント
- [add-test-spec SKILL](../../../.github/skills/add-test-spec/SKILL.md) - テスト追加手順
