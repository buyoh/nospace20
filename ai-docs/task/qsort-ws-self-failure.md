# test_example_qsort_ws_self 失敗調査

## 概要

Fix A および Fix B の実装により、5件の失敗テストのうち4件が解決したが、
`test_example_qsort_ws_self` のみが依然として失敗している。

## 現在の状況

**テスト名**: test_example_qsort_ws_self  
**テストパス**: examples/e1-00-qsort  
**失敗パターン**: 出力不一致  
**期待される出力**: "1 1 2 3 4 5 9 "  
**実際の出力**: ""（空）

## 原因特定済み (2026-02-17)

**Bug C: while ループ本体のブロック値がスタックに蓄積する問題**

`generate_while_expression` がループ本体に `generate_block` を使用しているが、
`generate_block` が末尾で生成する `push 0`（ブロック式値）がイテレーションごとに
スタックに蓄積し、`generate_local_deallocate` でのスタック不整合を引き起こしていた。

詳細な分析と修正設計は [fix-while-loop-stack-leak.md](../done-task/fix-while-loop-stack-leak.md) を参照。

## Fix C 実装後の状況 (2026-02-17)

Fix C を実装した結果、出力が変化:
- 修正前: "" (空)
- 修正後: "0 0 0 1 1 4 7 " (不正確だが空ではない)
- 期待値: "1 1 2 3 4 5 9 "

部分的な改善が見られるが、依然として正しい出力は得られていない。
他の 114/115 の ws_self テストは成功している。

**現在の仮説**: qsort 特有の複雑な再帰構造や配列操作に関連する別のバグが存在する可能性。
さらなる調査が必要。

## 関連ドキュメント

- [fix-while-loop-stack-leak.md](fix-while-loop-stack-leak.md) - 修正設計（Fix C）
- [remaining-ws-self-failures.md](remaining-ws-self-failures.md) - Fix A/B の実装記録
