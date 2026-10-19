# コードレビュー 2026-10

実施日: 2026-10-18

ソースコード・テストコード・設定ファイルを対象に、2 エージェントで並行レビュー（フロントエンド/API/設定、バックエンド/テスト）を行い統合した結果。
観点: 一般的な設計原則、安全性・エラー処理・フェイルセーフ、外部公開（WASM / CI）のセキュリティ。
[code-review-2026-04.md](../code-review-2026-04.md) および `done-task/code-design-review/`・`done-task/api-cleanup/` で記録済みの指摘は除外。

## 指摘一覧

| # | 重要度 | ドキュメント | 概要 |
|---|---|---|---|
| 01 | high | [01-parser-infinite-loop.md](01-parser-infinite-loop.md) | **完了**（[fix-parser-loop-ws-number-overflow](../../done-task/fix-parser-loop-ws-number-overflow/README.md)） 引数リスト中の不正トークンで構文解析が無限ループ・メモリ無制限消費（`f(;` で再現、WASM でタブがフリーズ） |
| 02 | high | [02-block-nested-function-misresolution.md](02-block-nested-function-misresolution.md) | 設計済み（[block-nested-function](../block-nested-function/README.md)） ブロック内で宣言した関数の呼び出しが `__main` 等の別関数に解決される（誤コード生成） |
| 03 | high | [03-ws-vm-arithmetic-overflow.md](03-ws-vm-arithmetic-overflow.md) | **完了**（[fix-parser-loop-ws-number-overflow](../../done-task/fix-parser-loop-ws-number-overflow/README.md)） WhitespaceVM の算術オーバーフローで panic、バックエンド間で挙動不一致 |
| 04 | high | [04-ws-parser-number-label.md](04-ws-parser-number-label.md) | **完了**（[fix-parser-loop-ws-number-overflow](../../done-task/fix-parser-loop-ws-number-overflow/README.md)） WS パーサの数値オーバーフロー panic、`i64::MIN` 非往復、ラベル衝突 |
| 05 | high | [05-runtime-error-panic.md](../../done-task/code-review-2026-10/05-runtime-error-panic.md) | **完了**（[runtime-robustness-2026-10](../../done-task/runtime-robustness-2026-10/README.md)） インタプリタの実行時エラー（0 除算・assert・不正アクセス・I/O）を panic で表現 |
| 06 | high | [06-interpreter-scope-memory-leak.md](06-interpreter-scope-memory-leak.md) | インタプリタの内部スコープ領域が解放されずメモリ線形増加 |
| 07 | high | [07-recursion-depth-limit.md](../../done-task/code-review-2026-10/07-recursion-depth-limit.md) | **完了**（[runtime-robustness-2026-10](../../done-task/runtime-robustness-2026-10/README.md)） パーサ・インタプリタに再帰深度上限がなくスタックオーバーフロー abort |
| 08 | medium | [08-array-size-overflow.md](08-array-size-overflow.md) | 配列・struct サイズ計算の未チェック演算 |
| 09 | medium | [09-condition-opt-overflow-semantics.md](09-condition-opt-overflow-semantics.md) | condition_opt がオーバーフロー時に意味を変える |
| 10 | medium | [10-geti-input-inconsistency.md](10-geti-input-inconsistency.md) | `__geti` の入力解釈が三者で食い違う |
| 11 | medium | [11-putc-utf8-output.md](11-putc-utf8-output.md) | `__putc` が 128〜255 を UTF-8 2 バイトで出力 |
| 12 | medium | [12-wasm-compile-opt-mismatch.md](../../done-task/code-review-2026-10/12-wasm-compile-opt-mismatch.md) | **完了**（[runtime-robustness-2026-10](../../done-task/runtime-robustness-2026-10/README.md)） WASM compile が peephole を適用しない、パス表の 3 重管理 |
| 13 | medium | [13-deploy-token-exposure.md](13-deploy-token-exposure.md) | デプロイで書き込みトークンを外部コード実行に晒す |
| 14 | medium | [14-unregistered-tests.md](14-unregistered-tests.md) | どこからも実行されないテストリソース |
| 15 | low | [15-ws-vm-instruction-inconsistency.md](15-ws-vm-instruction-inconsistency.md) | 設計済み（[ws-vm-copy-index-conversion](../ws-vm-copy-index-conversion.md) / [ws-slide-instruction](../ws-slide-instruction/README.md) / [ws-vm-input-via-heap-store](../ws-vm-input-via-heap-store.md)） WS VM の Copy / Slide / 入力命令の不整合 |
| 16 | low | [16-cli-error-output.md](16-cli-error-output.md) | 設計済み（[cli-errors-and-doccomments](../cli-errors-and-doccomments/README.md)） CLI のエラー出力先不統一・stdin エラー握りつぶし・バージョン不一致 |
| 17 | low | [17-tree-parser-duplication.md](17-tree-parser-duplication.md) | 設計済み（[tree-parser-dedup](../tree-parser-dedup/README.md)） tree_parser の重複コード・巨大モジュール |
| 18 | low | [18-location-doc-and-missing-doccomments.md](18-location-doc-and-missing-doccomments.md) | 設計済み（[cli-errors-and-doccomments](../cli-errors-and-doccomments/README.md)。EOF 位置は [tree-parser-dedup](../tree-parser-dedup/README.md)） 位置情報の単位誤記・ドキュメントコメント不足 |
| 19 | low | [19-dead-code.md](19-dead-code.md) | 死にコード・誤解を招く実装 |
| 20 | low | [20-test-harness-and-ci.md](../../done-task/code-review-2026-10/20-test-harness-and-ci.md) | **完了** テストハーネス・CI の品質 |

## 推奨対応順

1. 01, 02, 07（パーサ部分）: ユーザー入力だけでハング・誤コード生成・abort。WASM エディタでも発生
2. 03, 04, 05, 06: 信頼できないプログラムで panic / abort / 資源枯渇
3. 03 の仕様決定と合わせて 09、続いて 08, 10, 11, 12, 13
4. 14 以降: テスト・保守性

## 関連

- 03 / 08 / 09: 整数オーバーフロー仕様（spec 未定義）を一括で決めると効率が良い
- 05 / 07: CLI を NospaceVM に統一すれば、Result 化の対象を片方に絞れる
- 01 / 17: 引数ループの共通化で同時に解消
