# Task

現在作業中のタスク・進捗を記録するディレクトリ。

## ドキュメント

### 未実装機能の追跡

- [unimplemented-variable-features.md](unimplemented-variable-features.md) - 未実装の変数関連機能（初期値指定、final/const変数）
- [unimplemented-type-system.md](unimplemented-type-system.md) - 未実装の型システム（int, void, function, tuple）
- [unimplemented-compiler-features.md](unimplemented-compiler-features.md) - 未実装のコンパイラ機能（compiler, grayspace）
- [technical-debt.md](technical-debt.md) - コード内の技術的負債（Clone derive削除、識別子管理改善）
- [error-message-improvement.md](error-message-improvement.md) - エラーメッセージ型の改善（Cow<'static, str> 導入設計）
- [implement-getcv-builtin.md](implement-getcv-builtin.md) - __getcv 組み込み関数の実装（変数アドレスへの入力）
- [implement-multi-variable-declaration.md](implement-multi-variable-declaration.md) - 複数変数宣言の実装（let:a, b;）
- [implement-compound-assignment-operators.md](implement-compound-assignment-operators.md) - 複合代入演算子の実装（+=, -=, *=, /=, %=）

### アクティブなタスク

- [block-scope-expression.md](block-scope-expression.md) - ブロックスコープ式 `{ ... }` の実装（独立したスコープ機能）
- [error-specification/](error-specification/) - エラー仕様のドキュメント化と自動生成手段の検討
- [reference-dereference/](reference-dereference/) - 参照(`&`)・デリファレンス(`*`)演算子の実装（spec.md 2.7）
- [whitespace-interpreter/](whitespace-interpreter/) - Whitespace インタプリタ（明示的スタックマシン、中断可能実行）
- [suspendable-interpreter/](suspendable-interpreter/) - インタプリタ中断・再開機能（N ステップ実行→一時停止→再開）
- [wasm-build/](wasm-build/) - Rust → WebAssembly ビルド（ランタイム WASM 化、Phase A: WS ステップ実行、Phase B: nospace ステップ実行）
- [wasm-js-compiler/](wasm-js-compiler/) - nospace → WASM / JavaScript コンパイラ設計・実装
- [integration-test-design.md](integration-test-design.md) - 結合テスト設計・計画
- [whitespace-integration-test.md](whitespace-integration-test.md) - Whitespace コンパイラ統合テスト設計
- [compiler/](compiler/) - nospace → Whitespace コンパイラ調査 (旧実装分析)
- [self-compiler/](self-compiler/) - セルフコンパイラ用縮小仕様（nospace-core）の設計
- [scope/](scope/) - スコープ機能の実装計画
- [array-implementation/](array-implementation/) - 配列（spec.md §4.2）・文字列リテラル（§4.3）の実装設計（5フェーズ: 構文解析→意味解析→インタプリタ→WSコンパイラ→文字列）

## 現在のタスク

### アクティブ

- [ignore-debug-builtins.md](ignore-debug-builtins.md) - __assert/__trace 無視オプション（--ignore-debug CLI フラグ追加）

### 完了済み (done-task/ に移動)

- [unit-test-interpreter.md](../done-task/unit-test-interpreter.md) - interpreter ユニットテスト追加（組み込み関数・演算子・制御フロー、25件追加） (2026-02-10完了)
- [interpreter-split.md](../done-task/interpreter-split.md) - interpreter モジュールのファイル分割（mod.rs → types/environment/exec） (2026-02-07完了)
  - mod.rs（622行）を4ファイルに分割
  - 責務の明確化: 型定義（types.rs）、環境管理（environment.rs）、実行ロジック（exec.rs）
  - 全72テストが成功、外部インターフェースに影響なし
  - 将来の拡張（ユニットテスト、suspendable interpreter）の土台を構築
- [unimplemented-syntax-features.md](../done-task/unimplemented-syntax-features.md) - 未実装の構文と式の機能 (2026-02-07完了)
  - 16進数リテラル（数値 0xFF、文字 \xHH）を実装
  - if/while 式の戻り値は既に実装済みであることを確認
  - return なし関数の戻り値仕様を確定（0を返す）
  - 全テスト成功 (88 unit tests, 72 integration tests)
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
