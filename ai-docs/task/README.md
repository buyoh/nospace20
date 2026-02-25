# Task

現在作業中のタスク・進捗を記録するディレクトリ。

## ドキュメント

### 未実装機能の追跡

- [unimplemented-variable-features.md](unimplemented-variable-features.md) - 未実装の変数関連機能（初期値指定、final/const変数）
- [unimplemented-type-system.md](unimplemented-type-system.md) - 未実装の型システム（int, void, function, tuple）
- [unused-code-cleanup.md](unused-code-cleanup.md) - 未使用コードの整理（17件の警告調査・分類済み、Phase 1-3 の対処方針）
- [fix-build-warnings.md](fix-build-warnings.md) - ビルド時 warning の調査・修正計画（14件の warning 分類・修正方針、2026-02-25 調査）
- [symbol-table-design.md](symbol-table-design.md) - デバッグ用シンボルテーブル設計（識別子のインデックス化・文字列分離）- ステップ1-4完了、ステップ5-6の詳細設計完了 (2026-02-17)
  - [symbol-table-impl/](symbol-table-impl/) - ステップ5・6の詳細設計
- [error-message-improvement.md](error-message-improvement.md) - エラーメッセージ型の改善（Cow<'static, str> 導入設計）
- [implement-getcv-builtin.md](implement-getcv-builtin.md) - __getcv 組み込み関数の実装（変数アドレスへの入力）
- [implement-multi-variable-declaration.md](implement-multi-variable-declaration.md) - 複数変数宣言の実装（let:a, b;）
- [implement-compound-assignment-operators.md](implement-compound-assignment-operators.md) - 複合代入演算子の実装（+=, -=, *=, /=, %=）

### アクティブなタスク

- [internal-type-system/](internal-type-system/) - 内部型システム（int / void）の導入（明示的型定義なし、while→void、else なし if→void、関数戻り値推論、semantic_analyzer での型チェック）
- [whitespace-duplicate-label-check.md](whitespace-duplicate-label-check.md) - Whitespace 重複ラベル定義のエラー検出（パース時の重複チェック、ws_parse_error テストタイプ追加）
- [fix-function-arg-count-check.md](fix-function-arg-count-check.md) - 関数呼び出し引数数チェックの実装（意味解析でのコンパイルエラー検出）
- [index-operator-on-non-array.md](index-operator-on-non-array.md) - 非配列変数への `[]` 演算子適用（`arr[i]` は `*(&arr + i)` と同義、semantic_analyzer のチェック緩和）
- [std-ext-debug-whitespace/](std-ext-debug-whitespace/) - `--std-ext debug` による Whitespace デバッグ拡張 API 対応（コンパイラ・VM・API の3 Phase 設計）
- [error-test-coverage/](error-test-coverage/) - エラーテストケース網羅性向上（18件のテスト追加計画、字句解析6件・構文解析7件・意味解析5件）
- [multi-error-reporting.md](multi-error-reporting.md) - 意味解析における複数エラー報告（semantic_analyzer で複数箇所のコンパイルエラーを収集・表示）
- [duplicate-function-check.md](duplicate-function-check.md) - 同一スコープ内の関数重複定義の検出（semantic error）
- [block-scope-expression.md](block-scope-expression.md) - ブロックスコープ式 `{ ... }` の実装（独立したスコープ機能）
- [reference-dereference-compiler-ws.md](reference-dereference-compiler-ws.md) - 参照(`&`)・デリファレンス(`*`)のWhitespaceコンパイラ実装（Phase 4、インタプリタは完了済み）
- [whitespace-static-variable-issue.md](whitespace-static-variable-issue.md) - Whitespace コンパイラでの static 変数サポート（static 変数をグローバルヒープに配置、初期化コード生成）
- [suspendable-interpreter/](suspendable-interpreter/) - インタプリタ中断・再開機能（N ステップ実行→一時停止→再開、Phase 5: nospace ステップ実行 WASM API）
- [wasm-js-compiler/](wasm-js-compiler/) - nospace → WASM / JavaScript コンパイラ設計・実装
- [wasm-compile-error-location.md](wasm-compile-error-location.md) - WASM版コンパイルエラーの位置情報表示（compiler_ws の CompileError に SourceLocation 付与）
- [fix-e0-00-puts-test.md](fix-e0-00-puts-test.md) - e0-00-puts テスト失敗の修正（__puti デバッグ行の除去）
- [integration-test-design.md](integration-test-design.md) - 結合テスト設計・計画
- [whitespace-integration-test.md](whitespace-integration-test.md) - Whitespace コンパイラ統合テスト設計
- [whitespace-interpreter-tests.md](whitespace-interpreter-tests.md) - Whitespace インタプリタ直接テスト設計（resources/tests_ws/, WSA記法, 29テストケース計画）
- [self-compiler/](self-compiler/) - セルフコンパイラ用縮小仕様（nospace-core）の設計
- [rename-trace-attribute.md](rename-trace-attribute.md) - テスト check.json の `trace` 属性名改善（`trace` → `trace_hit_counts`）
- [wasm-vm-interactive-stdin/](wasm-vm-interactive-stdin/) - WASM WhitespaceVM の interactive stdin 一時停止機能（InputChar/InputNumber でバッファ不足時に WaitingForInput を返す）
- [ws-profiler/](ws-profiler/) - Whitespace VM プロファイラ（実行ステップ数・メモリアクセス範囲の統計収集、YAML 出力スクリプト）
- [ws-profiler-html-report/](ws-profiler-html-report/) - プロファイラ HTML レポート（ws_profiler JSON 出力追加 + HTML サマリ・比較レポート生成スクリプト）
- [review-tree-parser-statement.md](review-tree-parser-statement.md) - tree_parser/statement/mod.rs コードレビュー（リファクタ4件・品質改善5件）
- [add-elsif-keyword.md](add-elsif-keyword.md) - elsif キーワードの追加（AST 不変方式、トークン追加・パーサー修正・else:if: 廃止、Step 1-7）

## 現在のタスク

### アクティブ

- [alloc-reuse-efficiency-tests.md](alloc-reuse-efficiency-tests.md) - メモリアロケータ再利用効率テスト（VM ヒープ直接検査によるユニットテスト、Bump 2 件 + FSBA 9 件）
- [ignore-debug-builtins.md](ignore-debug-builtins.md) - __assert/__trace 無視オプション（--ignore-debug CLI フラグ追加）
- [fix-block-scope-offset/](fix-block-scope-offset/) - ブロックスコープ変数のヒープオフセット衝突修正設計（Bug D: scope_depth 無視による変数衝突）
- [mnemonic-wsc-compat.md](mnemonic-wsc-compat.md) - ニーモニック出力を wsc 形式に近づける（命令名7件リネーム、ラベル宣言形式変更、インデント追加）
- [optional-trailing-semicolon.md](optional-trailing-semicolon.md) - ブロック末尾のセミコロン省略（最後のステートメントで `;` を省略可能にする設計）
- [split-build-rs.md](split-build-rs.md) - build.rs の分割リファクタリング（523行→src_build/ ディレクトリへモジュール分割）

### 完了済み (done-task/ に移動)

- [memory-allocator/](../done-task/memory-allocator/) - メモリアロケータ実装完了（Phase 1-5 全て完了：`--std-ext alloc`、AllocRuntime trait、分離テスト、FSBA+First-Fit、`__alloc`/`__free` 組み込み関数、全916テスト通過）(2026-02-27完了)
- [qsort-ws-self-failure.md](../done-task/qsort-ws-self-failure.md) - test_example_qsort_ws_self 失敗調査・修正（Bug C/D/E を解決、ws_self 全テスト PASS、121件）(2026-02-17完了)
- [fix-ws-self-label-duplication.md](../done-task/fix-ws-self-label-duplication.md) - compiler_ws ラベル ID 重複バグ修正（10/15件成功、256 passed; 5 failed）(2026-02-17完了)
- [whitespace-test-failures-investigation.md](../done-task/whitespace-test-failures-investigation.md) - Whitespace インタプリタテスト失敗調査（全39テスト成功確認、12件の失敗すべて解決）(2026-02-17完了)
- [fix-whitespace-vm-test-failures.md](../done-task/fix-whitespace-vm-test-failures.md) - WhitespaceVM テスト失敗修正（WSAエンコーディング誤り11箇所 + マニフェストパス誤り4件）(2026-02-17完了)
- [wsc-cross-validation.md](../done-task/wsc-cross-validation.md) - wsc による WSA テストケースのクロスバリデーション（外部 Whitespace インタプリタでテストケースの正当性を検証、全39テスト合格）(2026-02-16完了)
- [wasm-build/](../done-task/wasm-build/) - WASM ビルド完全完了（Phase 0/1/A/3 + サイズ最適化、269KB → 198KB）(2026-02-16完了)
- [wasm-build-phase0-1-a-3-completion.md](../done-task/wasm-build-phase0-1-a-3-completion.md) - WASM ビルド Phase 0/1/A/3 完了（ビルド基盤、run/compile API、Whitespace VM ステップ実行、テスト環境）(2026-02-12完了)
- [error-specification/](../done-task/error-specification/) - エラー仕様のドキュメント化と自動生成手段の検討（調査・設計フェーズ完了：6ドキュメント、計1600行以上のエラー仕様を記録）(2026-02-16完了)
- [array-implementation/](../done-task/array-implementation/) - 配列（spec.md §4.2）・文字列リテラル（§4.3）の実装（Phase 1-5 全て完了：構文解析→意味解析→インタプリタ→WSコンパイラ→文字列）(2026-02-13完了)

- [identifier-management-improvement.md](../done-task/identifier-management-improvement.md) - 識別子管理改善設計（Variable.identifier 削除、IdentifierInfo 型安全化）(2026-02-11完了)
- [scope/](../done-task/scope/) - スコープ機能の実装（Phase 1-5 全て完了：ブロックスコープ、識別子解決、グローバル変数、static変数、ネスト関数）(2026-02-11完了)
- [reference-dereference-interpreter-implementation.md](../done-task/reference-dereference-interpreter-implementation.md) - 参照(`&`)・デリファレンス(`*`)の実装完了（Phase 1-3: token_parser, tree_parser, semantic_analyzer, interpreter）(2026-02-10完了)
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
