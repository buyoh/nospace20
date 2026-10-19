# Task

現在作業中のタスク・進捗を記録するディレクトリ。

## ドキュメント

- [code-review-2026-10/](code-review-2026-10/) - コードレビュー 2026-10（high 7件: パーサ無限ループ、ブロック内関数の誤解決、WS VM/パーサのオーバーフロー panic、実行時エラーの panic、スコープ領域リーク、再帰深度無制限 / medium 7件 / low 6件）
- [block-nested-function/](block-nested-function/) - ブロックスコープ内の関数宣言（コードレビュー 2026-10 #02。ブロック内の関数呼び出しが `__main` 等に誤解決される不具合。A: 全ブロックスコープで関数宣言を意味解析エラーにする（3.0.0 PATCH）→ B: `FunctionTable` を式の変換を含む全経路で共有して許可し、ブロック越しの非 static 参照を境界判定で禁止（次バージョン MINOR））
- [ws-vm-run-mode-support/](ws-vm-run-mode-support/) - WhitespaceVM の機能追加（retire-direct-interpreter の前提。VM が使うメモリ（ヒープ・データスタック・戻り先のセル数）の上限 `max_memory_cells`、`__clog` の debug 拡張 API `-13` と専用 writer、`WasmWhitespaceVM` の debug 拡張反映バグ修正・`opt_passes`・`Display` エラー。NospaceVM の置換は行わない）
- [retire-direct-interpreter/](retire-direct-interpreter/) - 直接実行インタプリタの廃止（CLI `--mode run` を compile + WhitespaceVM に置き換え、NospaceVM / `WasmNospaceVM` / `interpret*` API を削除、ツリー走査インタプリタを `#[cfg(test)]` の `src/semantic_test/` に移して意味解析テストの正解器にする。ws-vm-run-mode-support の完了が前提）
- [tree-parser-dedup/](tree-parser-dedup/) - tree_parser の重複解消（コードレビュー 2026-10 #17。`ParserCursor` で Builder 共通処理・`parse_type_spec` を集約、引数リスト共通化、2010 行の statement/mod.rs を 7 ファイルに分割）
- [cli-errors-and-doccomments/](cli-errors-and-doccomments/) - CLI エラー出力の整理と位置情報・ドキュメントコメントの修正（コードレビュー 2026-10 #16・#18。エラーの stderr 統一、stdin 読み取りエラーの報告、`--version` を Cargo に連動、run モードの `--target`/`-o` を拒否、whitespace20 のエラーを `Display` 表示、`SourceLocation` 等が文字インデックスであることの明記と欠落 doc の追加）
- [array-size-overflow/](array-size-overflow/) - 配列・struct サイズ計算のオーバーフロー対策（コードレビュー 2026-10 #08。案 A のみ: パーサの `usize::try_from`、意味解析のスロット累積・struct サイズの checked 化、compiler_ws のフレーム・グローバル領域サイズの `i64` 検査。案 B（サイズ上限）は不採用とし、実施する場合の検査箇所を記録）
- [nested-block-function-and-static.md](nested-block-function-and-static.md) - ネストしたブロック内の関数定義・static 宣言の不具合（run・WS 両方で stack overflow / 初期化子消失。ルートブロック内の関数からのブロック変数参照も未確認。ws-root-block-variable-address の設計レビューで発見。未設計）
- [ws-static-init-frame-compaction.md](ws-static-init-frame-compaction.md) - WS コンパイラの static 初期化子用フレーム相当領域を、実際に参照される非 static 変数の分だけに縮小（正しさには影響しない改善。ws-root-block-variable-address のコードレビューで指摘）
- [nospace-vm-recursive-static.md](nospace-vm-recursive-static.md) - NospaceVM（`--mode run`）で再帰中の static 変数の更新が失われるバグ（正解器側の誤り。ws-root-block-variable-address のコードレビューで発見。未調査）
- [ws-vm-copy-index-conversion.md](ws-vm-copy-index-conversion.md) - WhitespaceVM Copy のインデックス変換修正（wasm32 での `as usize` 切り詰めを `usize::try_from` + `checked_add` に、負の n は `|n|` のまま予約扱いとして spec に明記。コードレビュー 2026-10 #15-1）
- [ws-slide-instruction/](ws-slide-instruction/) - Whitespace Slide 命令（v0.3）対応（`Instruction::Slide` 追加、パーサ・VM・プロファイラ対応、負の n はエラー予約（要ユーザー確認）、コンパイラでの利用は調査のみ。コードレビュー 2026-10 #15-2）
- [ws-vm-input-via-heap-store.md](ws-vm-input-via-heap-store.md) - WhitespaceVM 入力命令のヒープ書き込みを `heap_store` に一元化（`--std-ext debug` フックとプロファイラ統計を Store と揃える。コードレビュー 2026-10 #15-3）
- [code-review-2026-04.md](code-review-2026-04.md) - コードレビュー 2026-04（CI/CD信頼性、サプライチェーンリスク、unsafeコメント不足、WASM入力検証、ディレクトリ不一致、ワークフロー設計）
- [cleanup-phase-comments.md](cleanup-phase-comments.md) - Phase コメント整理（完了済みタスクの `Phase N:` 開発メモを削除・整理、カテゴリ A: 6件完全削除 / カテゴリ B: 32件プレフィックス削除）
- [template-functions.md](template-functions.md) - テンプレート関数（関数ポインタ代替、alias パラメータによるコンパイル時関数生成）
- [multi-error-reporting.md](multi-error-reporting.md) - 意味解析における複数エラー報告（semantic_analyzer で複数箇所のコンパイルエラーを収集・表示）
- [optional-trailing-semicolon.md](optional-trailing-semicolon.md) - ブロック末尾のセミコロン省略（最後のステートメントで `;` を省略可能にする設計）
- [add-elsif-keyword.md](add-elsif-keyword.md) - elsif キーワードの追加（AST 不変方式、トークン追加・パーサー修正・else:if: 廃止、Step 1-7）
- [compile-optimization-phase2/](compile-optimization-phase2/) - コンパイル最適化 Phase 2（代入文値破棄最適化、比較演算インライン化、未使用変数削除、ピープホール最適化）
- [fix-short-circuit-evaluation.md](fix-short-circuit-evaluation.md) - WSコンパイラ短絡評価バグ修正（`&&`/`||` が両辺を常に評価しており仕様違反）
- [tail-call-optimization.md](tail-call-optimization.md) - 末尾呼出し最適化（自己末尾再帰のループ変換、スタックオーバーフロー防止）
- [self-compiler/](self-compiler/) - セルフコンパイラ用縮小仕様（nospace-core）の設計
- [wasm-js-compiler/](wasm-js-compiler/) - nospace → WASM / JavaScript コンパイラ設計・実装
- [repeat-form1-upper-bound.md](repeat-form1-upper-bound.md) - repeat Form 1 の意味論変更（ループ回数→上限値、隠し変数廃止）
- [keyword-colon-token-design.md](keyword-colon-token-design.md) - Keyword トークンにコロンを内包する設計の検討（break/continue/return の不整合分析）
- [code-design-review/](code-design-review/) - コード設計レビュー残タスク（02-module-splitting のみ未着手。01,03,04,05,06 は [done-task](../done-task/code-design-review/) に移動済み）
- [fix-postfix-subscript-semantics.md](fix-postfix-subscript-semantics.md) - 後置添字演算子 `(expr)[i]` の脱糖仕様変更（`*(expr + i)` → `*(&(expr) + i)` へ統一、テスト改善）
- [separate-inline-tests.md](separate-inline-tests.md) - インラインテスト分離（19ファイルの `#[cfg(test)] mod tests` を別ファイルに分離、Step 1-3 段階実施）
- [temporary-array-literals.md](temporary-array-literals.md) - 一時配列リテラル（文字列・配列を式として直接記述、候補1: 文スコープ一時 vs 候補2: 無名static変数、設計比較・推奨）
- [20260311-array-and-parse-error-failures.md](20260311-array-and-parse-error-failures.md) - 配列系7件とparse_error 1件の失敗調査と修正設計（原因分離・最小修正方針・検証計画）
- [api-cleanup/](api-cleanup/) - API クリーンアップ（deprecated関数移行・WS共有型配置改善・エラー型統一の3タスク）
- [interpreter-allocator/](interpreter-allocator/) - インタプリタ メモリアロケータ（`__alloc`/`__free` 対応、グローバル・ローカル・static 変数の統一メモリモデル、解放済みアドレス検出）
- [template-self-recursion.md](template-self-recursion.md) - テンプレート関数の自己再帰サポート（ボディ内でテンプレート名の再帰呼び出しを許可、自己参照alias自動挿入）
- [wasm-error-details.md](wasm-error-details.md) - WASM API エラー詳細情報の追加（WasmError に details フィールドを追加し、CLI と同等のソースコード行・キャレット表示を返す）
- [address-zero-reserved.md](address-zero-reserved.md) - アドレス 0 の予約仕様追加（`__alloc` や変数アドレスが 0 を返さないことを明文化、ヌルポインタ相当として利用可能に）
- [improve-unexpected-token-error.md](improve-unexpected-token-error.md) - unexpected token エラーメッセージ改善（実際に得られたトークンの情報を表示、Token::describe() 追加）
- [postfix-subscript-operator.md](postfix-subscript-operator.md) - 汎用後置添字演算子（`(*next)[1]`等の任意式への`[expr]`適用、パース時脱糖方式、Step 1-4）
- [namespace/](namespace/) - 名前空間（スコープを作らない名前修飾機構、$アクセス、コンパイル時マングリング、Step 1-8）
