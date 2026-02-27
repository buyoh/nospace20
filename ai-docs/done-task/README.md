# Done Task

完了したタスクのアーカイブディレクトリ。

## 完了済みタスク

| ファイル | 概要 | 完了日 |
|----------|------|--------|
| [while-expression-to-statement/](while-expression-to-statement/) | while を式から文に変更（Expression::While → Statement::While、7ステップ設計・実装完了、全テストPASS） | 2026-02-27 |
| [compile-optimization/](compile-optimization/) | コンパイル時最適化全5パス完了（フレームワーク、条件式最適化、geti最適化、未使用関数削除、定数畳み込み、テスト59件） | 2026-02-27〜28 |
| [wasm-api-unify-std-extensions.md](wasm-api-unify-std-extensions.md) | WASM API の std_extensions パラメータ統一（compile/WasmWhitespaceVM の個別 bool → StdExtension[] 配列、getOptions() と形式一致） | 2026-02-26 |
| [compile-optimization/01-pass-framework.md](compile-optimization/01-pass-framework.md) | 最適化パスフレームワーク＋ExecExpression拡張（ConditionMode/InternalBuiltinFunctionKind導入、If/While署名変更、全バックエンド対応、テスト19件） | 2026-02-27 |
| [compile-optimization/02-pass-condition-opt.md](compile-optimization/02-pass-condition-opt.md) | if/while 条件式最適化（JumpIfZero/JumpIfNegative の直接利用、condition_opt パス実装、テスト12件追加、全テスト 983 passed） | 2026-02-27 |
| [compile-optimization/03-pass-geti-opt.md](compile-optimization/03-pass-geti-opt.md) | `__geti`/`__getc` 入力最適化（TEMP_PTR 経由排除、geti_opt パス実装、テスト7件追加、全テスト 990 passed） | 2026-02-27 |
| [compile-optimization/04-pass-dead-code.md](compile-optimization/04-pass-dead-code.md) | 未使用関数削除（BFS 到達可能性解析、Function::dummy() 置換、dead_code パス実装、テスト8件追加、全テスト 998 passed） | 2026-02-28 |
| [compile-optimization/05-pass-constant-folding.md](compile-optimization/05-pass-constant-folding.md) | 定数畳み込み（Operation2/Operation1/If/While の定数式を Factor に置換、constant_folding パス実装、テスト13件追加、全テスト 1011 passed） | 2026-02-27 |
| [wasm-compile-error-location.md](wasm-compile-error-location.md) | WASM版コンパイルエラーの位置情報表示（Phase 1完了：LocatedExecStatement、CompileError に SourceLocation 付与、全テストパス） | 2026-02-26 |
| [internal-type-system/](internal-type-system/) | 内部型システム（int/void）の導入（ValueType enum、戻り値型推論、void式の値使用検出、mixed return検出、テスト7件追加＋5件修正、全662テスト通過） | 2026-02-26 |
| [alloc-reuse-efficiency-tests.md](alloc-reuse-efficiency-tests.md) | メモリアロケータ再利用効率テスト（Bump 2件 + FSBA 9件、ヘルパー関数追加、全26テスト通過） | 2026-02-26 |
| [split-build-rs.md](split-build-rs.md) | build.rs の分割リファクタリング（523行→src_build/ ディレクトリへモジュール分割） | 2026-02-26 |
| [expression-location/](expression-location/) | 式レベル位置情報の導入（LocatedExpression / LocatedExecExpression、Phase 1-2 完了） | 2026-02-26 |
| [ws-profiler-html-report/](ws-profiler-html-report/) | プロファイラ HTML レポート（ws_profiler JSON 出力追加 + HTML サマリ・比較レポート生成スクリプト） | 2026-02-26 |
| [fix-build-warnings.md](fix-build-warnings.md) | ビルド時 warning 14件の調査・修正（W1-W14 全修正完了） | 2026-02-25 |
| [review-tree-parser-statement.md](review-tree-parser-statement.md) | tree_parser/statement コードレビュー（リファクタ4件・品質改善5件、全実装完了） | 2026-02-25 |
| [ws-profiler/](ws-profiler/) | Whitespace VM プロファイラ（実行ステップ数・メモリアクセス範囲の統計収集、JSON/YAML 出力） | 2026-02-25 |
| [wasm-vm-interactive-stdin/](wasm-vm-interactive-stdin/) | WASM WhitespaceVM の interactive stdin 一時停止機能（InputChar/InputNumber でバッファ不足時に WaitingForInput を返す） | 2026-02-24 |
| [memory-allocator/](memory-allocator/) | メモリアロケータ実装（Phase 1-5 全完了：`--std-ext alloc`、AllocRuntime trait、FSBA+First-Fit、`__alloc`/`__free` 組み込み関数、全916テスト通過） | 2026-02-27 |
| [mnemonic-wsc-compat.md](mnemonic-wsc-compat.md) | ニーモニック出力を wsc 形式に近づける（命令名7件リネーム、ラベル宣言形式変更、インデント追加） | 2026-02-18 |
| [whitespace-static-variable-issue.md](whitespace-static-variable-issue.md) | Whitespace コンパイラでの static 変数サポート（static 変数をグローバルヒープに配置、初期化コード生成） | 2026-02-18 |
| [strict-heap-mode/](strict-heap-mode/) | Whitespace VM strict-heap モード（Phase 1-6 + テスト修正完了：未初期化ヒープ検出、randomize、変数初期値未定義仕様変更、6テスト修正後 622 件全パス） | 2026-02-18 |
| [compiler-ws-duplicate-label-bug.md](compiler-ws-duplicate-label-bug.md) | コンパイラ生成コードの重複ラベルバグ修正（function_labelsキーをString→usizeに変更、同名関数シャドーイング対応） | 2026-02-18 |
| [compiler-implementation-record.md](compiler-implementation-record.md) | コンパイラ実装状況記録（Whitespace コンパイラほぼ完成、98.9%テスト成功率、grayspace未着手） | 2026-02-18 |
| [symbol-table-design.md](symbol-table-design.md) | デバッグ用シンボルテーブルによる識別子名管理の完全実装（ステップ1-6全て完了：静的ストレージのインデックスキー化、SymbolTable導入） | 2026-02-17 |
| [whitespace-interpreter/](whitespace-interpreter/) | Whitespace インタプリタ実装完了（Phase 1-2完了、wsc クロスバリデーション、39テスト全成功）| 2026-02-17 |
| [fix-block-scope-offset/](fix-block-scope-offset/) | ブロックスコープ変数のメモリアドレス衝突修正（Bug D: 変数衝突バグ、4件のテスト修正、273 passed; 4 failed）| 2026-02-17 |
| [std-ext-debug-whitespace/](std-ext-debug-whitespace/) | `--std-ext debug` による Whitespace デバッグ拡張 API 対応（コンパイラ・VM・API の3 Phase 設計・実装完了） | 2026-02-17 |
| [whitespace-duplicate-label-check.md](whitespace-duplicate-label-check.md) | Whitespace 重複ラベル定義のエラー検出（パース時の重複チェック、ws_parse_error テストタイプ追加） | 2026-02-17 |
| [fix-function-arg-count-check.md](fix-function-arg-count-check.md) | 関数呼び出し引数数チェックの実装（意味解析でのコンパイルエラー検出） | 2026-02-17 |
| [index-operator-on-non-array.md](index-operator-on-non-array.md) | 非配列変数への `[]` 演算子適用（`arr[i]` は `*(&arr + i)` と同義化、semantic_analyzer のチェック緩和） | 2026-02-17 |
| [symbol-table-impl/](symbol-table-impl/) | シンボルテーブル ステップ5・6 実装（静的ストレージのインデックスキー化、SymbolTable 導入） | 2026-02-17 |
| [whitespace-self-test-failures.md](whitespace-self-test-failures.md) | whitespace-self テスト15件の失敗調査・修正完了（ラベル重複バグ、呼び出し規約バグ、whileループスタックリーク、ブロックスコープオフセット衝突）| 2026-02-18 |
| [fix-while-loop-stack-leak.md](fix-while-loop-stack-leak.md) | while ループ本体のスタックリーク修正（Bug C: Discard 追加、114/115 ws_self テスト成功）| 2026-02-17 |
| [add-whitespace-self-target.md](add-whitespace-self-target.md) | whitespace-self テストターゲット追加（独自 WhitespaceVM で nospace コンパイル結果を実行・検証）| 2026-02-17 |
| [whitespace-test-failures-investigation.md](whitespace-test-failures-investigation.md) | Whitespace インタプリタテスト失敗調査（全39テスト成功確認、12件の失敗すべて解決）| 2026-02-17 |
| [fix-whitespace-vm-test-failures.md](fix-whitespace-vm-test-failures.md) | WhitespaceVM テスト失敗修正（WSAエンコーディング誤り11箇所 + マニフェストパス誤り4件、全39テスト合格）| 2026-02-17 |
| [rename-trace-attribute.md](rename-trace-attribute.md) | テスト check.json の `trace` 属性名改善（`trace` → `trace_hit_counts`） | 2026-02-16 |
| [error-test-coverage/](error-test-coverage/) | エラーテストカバレッジ향上（18件のテスト追加：字句解析6件・構文解析7件・意味解析5件） | 2026-02-16 |
| [duplicate-function-check.md](duplicate-function-check.md) | 同一スコープ内の関数重複定義の検出（semantic error）実装 | 2026-02-16 |
| [whitespace-interpreter-tests.md](whitespace-interpreter-tests.md) | Whitespace インタプリタ直接テスト設計・実装（resources/tests_ws/, WSA記法, 29テストケース） | 2026-02-16 |
| [whitespace-integration-test.md](whitespace-integration-test.md) | Whitespace コンパイラ統合テスト設計・実装 | 2026-02-16 |
| [wsc-cross-validation.md](wsc-cross-validation.md) | wsc による WSA テストケースのクロスバリデーション（外部インタプリタで全39テストケースの正当性を検証、自前VM12件のバグ特定）| 2026-02-16 |
| [wasm-build/](wasm-build/) | WASM ビルド完全完了（Phase 0/1/A/3 + サイズ最適化、269KB → 198KB, 26%削減）| 2026-02-16 |
| [wasm-build-size-optimization.md](wasm-build-size-optimization.md) | WASM サイズ最適化完了レポート（Cargo.toml + wasm-opt、198KB / gzip: 78.3KB）| 2026-02-16 |
| [wasm-build-phase0-1-a-3-completion.md](wasm-build-phase0-1-a-3-completion.md) | WASM ビルド Phase 0/1/A/3 完了（ビルド基盤、run/compile API、Whitespace VM ステップ実行、テスト環境）| 2026-02-12 |
| [array-implementation/](array-implementation/) | 配列（docs/spec.md §4.2）・文字列リテラル（§4.3）の実装（Phase 1-5全て完了：構文解析→意味解析→インタプリタ→WSコンパイラ→文字列） | 2026-02-13 |
| [unused-code-cleanup.md](unused-code-cleanup.md) | 未使用コードの整理（17件の警告調査・分類、Phase 1-3 対処方針策定済み） | 2026-02-11 |
| [block-scope-expression.md](block-scope-expression.md) | ブロックスコープ式 `{ ... }` の実装（独立したスコープ機能、インタプリタ・コンパイラ対応） | 2026-02-15 |
| [reference-dereference-compiler-ws.md](reference-dereference-compiler-ws.md) | 参照(`&`)・デリファレンス(`*`)のWhitespaceコンパイラ実装（Phase 4 完了、インタプリタは 2026-02-10 完了） | 2026-02-13 |
| [identifier-management-improvement.md](identifier-management-improvement.md) | 識別子管理の改善（Variable.identifierフィールド削除、IdentifierInfo型安全化、全Phase完了） | 2026-02-11 |
| [scope/](scope/) | スコープ機能の実装（Phase 1-5全て完了：ブロックスコープ、識別子解決、グローバル変数、static変数、ネスト関数） | 2026-02-11 |
| [reference-dereference-interpreter-implementation.md](reference-dereference-interpreter-implementation.md) | 参照(`&`)・デリファレンス(`*`)演算子の実装（Phase 1-3完了: token_parser, tree_parser, semantic_analyzer, interpreter、全テストPASS） | 2026-02-10 |
| [function-args-identifier-resolution-completed.md](function-args-identifier-resolution-completed.md) | Function構造体のargs(Vec<String>)フィールド削除（識別子解決完全化、arg_indicesのみ使用） | 2026-02-10 |
| [interpret-func-with-io-global-variable-bug.md](interpret-func-with-io-global-variable-bug.md) | interpret_func_with_io がグローバル変数を初期化しないバグ修正（interpret_global 呼び出し追加） | 2026-02-10 |
| [unit-test-interpreter.md](unit-test-interpreter.md) | interpreter ユニットテスト追加完了（組み込み関数・演算子・制御フロー、計25件追加） | 2026-02-10 |
| [implement-compound-assignment-operators.md](implement-compound-assignment-operators.md) | 複合代入演算子の実装（+=, -=, *=, /=, %=、全バックエンド対応） | 2026-02-09 |
| [ignore-debug-builtins-implementation.md](ignore-debug-builtins-implementation.md) | `--ignore-debug` オプション実装完了（`__assert`/`__trace` を no-op 化、ignore_debug_test タイプ追加） | 2026-02-08 |
| [ignore-debug-builtins.md](ignore-debug-builtins.md) | `--ignore-debug` オプション設計（`__assert`/`__trace` 無視 CLI フラグ設計） | 2026-02-08 |
| [error-message-improvement.md](error-message-improvement.md) | エラーメッセージ型の改善設計（`Cow<'static, str>` 導入設計） | 2026-02-08 |
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
