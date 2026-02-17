# Done Task

完了したタスクのアーカイブディレクトリ。

## 完了済みタスク

| ファイル | 概要 | 完了日 |
|----------|------|--------|
| [compiler-ws-duplicate-label-bug.md](compiler-ws-duplicate-label-bug.md) | コンパイラ生成コードの重複ラベルバグ修正（function_labelsキーをString→usizeに変更、同名関数シャドーイング対応） | 2026-02-18 |
| [compiler-implementation-record.md](compiler-implementation-record.md) | コンパイラ実装状況記録（Whitespace コンパイラほぼ完成、98.9%テスト成功率、grayspace未着手） | 2026-02-18 |
| [symbol-table-design.md](symbol-table-design.md) | デバッグ用シンボルテーブルによる識別子名管理の完全実装（ステップ1-6全て完了：静的ストレージのインデックスキー化、SymbolTable導入） | 2026-02-17 |
| [whitespace-interpreter/](whitespace-interpreter/) | Whitespace インタプリタ実装完了（Phase 1-2完了、wsc クロスバリデーション、39テスト全成功）| 2026-02-17 |
| [fix-block-scope-offset/](fix-block-scope-offset/) | ブロックスコープ変数のメモリアドレス衝突修正（Bug D: 変数衝突バグ、4件のテスト修正、273 passed; 4 failed）| 2026-02-17 |
| [whitespace-self-test-failures.md](whitespace-self-test-failures.md) | whitespace-self テスト15件の失敗調査・修正完了（ラベル重複バグ、呼び出し規約バグ、whileループスタックリーク、ブロックスコープオフセット衝突）| 2026-02-18 |
| [fix-while-loop-stack-leak.md](fix-while-loop-stack-leak.md) | while ループ本体のスタックリーク修正（Bug C: Discard 追加、114/115 ws_self テスト成功）| 2026-02-17 |
| [add-whitespace-self-target.md](add-whitespace-self-target.md) | whitespace-self テストターゲット追加（独自 WhitespaceVM で nospace コンパイル結果を実行・検証）| 2026-02-17 |
| [whitespace-test-failures-investigation.md](whitespace-test-failures-investigation.md) | Whitespace インタプリタテスト失敗調査（全39テスト成功確認、12件の失敗すべて解決）| 2026-02-17 |
| [fix-whitespace-vm-test-failures.md](fix-whitespace-vm-test-failures.md) | WhitespaceVM テスト失敗修正（WSAエンコーディング誤り11箇所 + マニフェストパス誤り4件、全39テスト合格）| 2026-02-17 |
| [wsc-cross-validation.md](wsc-cross-validation.md) | wsc による WSA テストケースのクロスバリデーション（外部インタプリタで全39テストケースの正当性を検証、自前VM12件のバグ特定）| 2026-02-16 |
| [wasm-build/](wasm-build/) | WASM ビルド完全完了（Phase 0/1/A/3 + サイズ最適化、269KB → 198KB, 26%削減）| 2026-02-16 |
| [wasm-build-size-optimization.md](wasm-build-size-optimization.md) | WASM サイズ最適化完了レポート（Cargo.toml + wasm-opt、198KB / gzip: 78.3KB）| 2026-02-16 |
| [wasm-build-phase0-1-a-3-completion.md](wasm-build-phase0-1-a-3-completion.md) | WASM ビルド Phase 0/1/A/3 完了（ビルド基盤、run/compile API、Whitespace VM ステップ実行、テスト環境）| 2026-02-12 |
| [array-implementation/](array-implementation/) | 配列（spec.md §4.2）・文字列リテラル（§4.3）の実装（Phase 1-5全て完了：構文解析→意味解析→インタプリタ→WSコンパイラ→文字列） | 2026-02-13 |
| [identifier-management-improvement.md](identifier-management-improvement.md) | 識別子管理の改善（Variable.identifierフィールド削除、IdentifierInfo型安全化、全Phase完了） | 2026-02-11 |
| [scope/](scope/) | スコープ機能の実装（Phase 1-5全て完了：ブロックスコープ、識別子解決、グローバル変数、static変数、ネスト関数） | 2026-02-11 |
| [reference-dereference-interpreter-implementation.md](reference-dereference-interpreter-implementation.md) | 参照(`&`)・デリファレンス(`*`)演算子の実装（Phase 1-3完了: token_parser, tree_parser, semantic_analyzer, interpreter、全テストPASS） | 2026-02-10 |
| [function-args-identifier-resolution-completed.md](function-args-identifier-resolution-completed.md) | Function構造体のargs(Vec<String>)フィールド削除（識別子解決完全化、arg_indicesのみ使用） | 2026-02-10 |
| [interpret-func-with-io-global-variable-bug.md](interpret-func-with-io-global-variable-bug.md) | interpret_func_with_io がグローバル変数を初期化しないバグ修正（interpret_global 呼び出し追加） | 2026-02-10 |
| [unit-test-interpreter.md](unit-test-interpreter.md) | interpreter ユニットテスト追加完了（組み込み関数・演算子・制御フロー、計25件追加） | 2026-02-10 |
| [builtin-functions-implementation.md](builtin-functions-implementation.md) | Whitespace コンパイラ ビルトイン関数の実装完了（__puti, __putc, __geti, __getc, __trace, __assert, __assert_not） | 2026-02-10 |
| [static_variable_initialization_limitation.md](static_variable_initialization_limitation.md) | static変数の未実装問題 → 調査の結果、既に正しく動作していることを確認（Phase 4で解決済み） | 2026-02-10 |
| [scope-phase4-static-variables.md](scope-phase4-static-variables.md) | static変数の永続化・初期化順序の実装（Phase 4完了） | 2026-02-10 |
| [scope-phase4-design.md](scope-phase4-design.md) | static変数実装の設計ドキュメント（Phase 4設計） | 2026-02-10 |
| [scope-analysis.md](scope-analysis.md) | スコープ機能全体の分析（Phase 1-4 概要） | 2026-02-10 |
| [scope-test-coverage-review.md](scope-test-coverage-review.md) | スコープ機能のテストカバレッジレビュー | 2026-02-10 |
| [fix-single-brace-panic.md](fix-single-brace-panic.md) | `}` だけのソースコードでのパニック修正（余剰トークンチェック、main関数事前チェック、エラーハンドリング改善） | 2026-02-10 |
| [whitespace-interpreter-phase1.md](whitespace-interpreter-phase1.md) | Whitespaceインタプリタ Phase 1 実装完了（パーサ、VM、CLI、全テスト成功） | 2026-02-08 |
| [interpreter-split-report.md](interpreter-split-report.md) | interpreterモジュールファイル分割完了報告（mod.rs→4ファイル、責務分離、全72テスト成功） | 2026-02-07 |
| [interpreter-split.md](interpreter-split.md) | interpreterモジュールファイル分割設計（types/environment/exec、将来拡張の土台） | 2026-02-07 |
| [hexadecimal-literals-implementation-report.md](hexadecimal-literals-implementation-report.md) | 16進数リテラル実装完了報告（数値0xFF、文字\xHH、全テスト成功） | 2026-02-07 |
| [unimplemented-syntax-features.md](unimplemented-syntax-features.md) | 未実装機能の実装完了（16進数リテラル、if/while式戻り値、return仕様確定） | 2026-02-07 |
| [block-scope-global-variables-implementation.md](block-scope-global-variables-implementation.md) | ブロックスコープ変数・グローバル変数・static変数・else if構文の実装完了 | 2026-02-07 |
| [compile-test-refactoring.md](compile-test-refactoring.md) | compile_test.rsリファクタリング完了（13件→1件に縮小、compile_errorテスト追加） | 2026-02-07 |
| [semantic-analyzer-error-handling-report.md](semantic-analyzer-error-handling-report.md) | Semantic Analyzerエラーハンドリング改善完了報告（panic!をResultに置換、位置情報付与） | 2026-02-07 |
| [semantic-analyzer-error-handling.md](semantic-analyzer-error-handling.md) | Semantic Analyzerエラーハンドリング改善設計（Result型への移行、位置情報付与） | 2026-02-07 |
| [multiple-io-test-cases-report.md](multiple-io-test-cases-report.md) | 複数の入出力テストケース対応完了報告（cases配列でIoTestCaseサポート） | 2026-02-07 |
| [multiple-io-test-cases.md](multiple-io-test-cases.md) | 複数の入出力テストケース対応設計（成功用の複数ケーステスト機能） | 2026-02-07 |
| [legacy-migration-phase3-report.md](legacy-migration-phase3-report.md) | 旧テストの移行Phase3完了報告（else:if構文サポート、I/Oテスト作成） | 2026-02-07 |
| [legacy-migration-phase3.md](legacy-migration-phase3.md) | 旧テストの移行Phase3タスク計画（else:if構文とI/Oテスト） | 2026-02-07 |
| [yaml-test-generation.md](yaml-test-generation.md) | YAMLベースのテスト自動生成（test-manifest.yaml による自動生成機構） | 2026-02-04 |
| [cli-compile-options.md](cli-compile-options.md) | CLIコンパイルオプション設計・実装（--std, --mode, --target オプション） | 2026-02-04 |
| [code-structure-refactoring.md](code-structure-refactoring.md) | ソースコード構成のリファクタリング（型整理、マクロ集約、可視性統一） | 2026-02-04 |
| [scope-phase2-identifier-resolution.md](scope-phase2-identifier-resolution.md) | スコープ機能Phase2実装報告（変数アクセスO(1)化、2パス解析） | 2026-02-05 |
| [phase2-identifier-resolution.md](phase2-identifier-resolution.md) | スコープ機能Phase2設計ドキュメント（識別子の事前解決） | 2026-02-05 |
| [scope-phase1-block-scope.md](scope-phase1-block-scope.md) | スコープ機能Phase1設計ドキュメント（ブロックスコープ変数の最小実装） | 2026-02-03 |
| [scope-phase1-implementation.md](scope-phase1-implementation.md) | スコープ機能Phase1実装報告（ブロックスコープ、シャドウイング） | 2026-02-03 |
| [cli-improvements.md](cli-improvements.md) | CLI改善（ファイル引数、デバッグフラグ、ヘルプ・バージョン表示） | 2026-02-01 |
| [migrate-legacy-tests.md](migrate-legacy-tests.md) | 旧テストの移行（I/Oビルトイン実装、27件のレガシーテスト移行） | 2026-02-01 |
| [unit-test-semantic-analyzer.md](unit-test-semantic-analyzer.md) | semantic_analyzer モジュールのユニットテスト追加（11件のテスト実装、モジュール分割） | 2026-02-01 |
| [unit-test-tree-parser.md](unit-test-tree-parser.md) | tree_parser モジュールのユニットテスト追加（37件のテスト実装） | 2026-02-01 |
| [unit-test-analysis.md](unit-test-analysis.md) | ユニットテスト分析レポート（Phase 1完了、タスク分離） | 2026-02 |
| [implement-new-features.md](implement-new-features.md) | 新機能実装（文字リテラル、論理演算子、剰余演算子、I/O関数） | 2026-02 |
| [test-categorization.md](test-categorization.md) | テストケースのカテゴリ分けと拡充 | 2026-01 |

## 注意

このディレクトリのドキュメントは更新されません。最新の情報は [task/](../task/README.md) を参照してください。
