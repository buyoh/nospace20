# 残りの _ws_self テスト失敗 (5件) 調査

## 概要

[fix-ws-self-label-duplication.md](fix-ws-self-label-duplication.md) の修正により、
15件の失敗テストのうち10件が成功に変わったが、残り5件が依然として失敗している。

これらのテストは、より複雑なラベル重複パターンまたは別の問題を抱えている可能性がある。

## 残りの失敗テスト (5件)

| # | テスト名 | 失敗パターン | 詳細 |
|---|---|---|---|
| 1 | test_example_fibonacci_ws_self | Suspended (無限ループ) | ステップ数上限超過 |
| 2 | test_example_qsort_ws_self | 出力不一致 | 期待値あり、実際は空出力 |
| 3 | test_legacy_014_ws_self | 出力不一致 | 期待: "1-12-2", 実際: "8-81-1" |
| 4 | test_legacy_015_ws_self | Suspended | ステップ数上限超過 |
| 5 | test_legacy_020_ws_self | 出力不一致 | 期待: "0111;1010;1010;0111;0101;", 実際: "0111;0111;1010;" |

## 仮説

### 仮説1: ネストした制御構造でのラベル重複

これらのテストは、複数の制御構造（if/else/while）がネストしているか、
より多くのラベルを使用しているため、単純な `sync_labels_from` では
すべてのラベル重複を解消できていない可能性がある。

### 仮説2: return 式内の制御構造

`generate_return()` は修正したが、return 式内に if/while 式が含まれる場合、
その式の評価時に子コンテキストでラベルが割り当てられる可能性がある。

現在の実装では `generate_return()` は `&mut CodeGenContext` を受け取るが、
`expression::generate_expression()` を呼び出すだけなので、
式の評価中に割り当てられたラベルは自動的に親コンテキストに反映される。

### 仮説3: 式内での関数呼び出しと制御構造の組み合わせ

関数呼び出し式の引数に制御構造が含まれる場合など、
より複雑なケースで問題が発生している可能性がある。

## 次のステップ

1. 各失敗テストのソースコードを確認
2. コンパイル結果（`--target mnemonic`）でラベル重複を確認
3. 独自VM でステップ実行し、どのラベルジャンプで問題が発生するか特定
4. 必要に応じて追加の修正を実施

## ステータス

- [x] 失敗テストのリスト作成
- [ ] 各テストのソースコード確認
- [ ] コンパイル結果の詳細分析
- [ ] 根本原因の特定
- [ ] 修正実装

## 関連ドキュメント

- [fix-ws-self-label-duplication.md](fix-ws-self-label-duplication.md) - 既に修正した10件のラベル重複バグ
- [whitespace-self-test-failures.md](whitespace-self-test-failures.md) - 元の15件の失敗調査
