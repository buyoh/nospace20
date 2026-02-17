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

## Bug D 特定 (2026-02-17)

**Bug D: ブロックスコープ変数のヒープオフセット衝突**

`CodeGenContext::get_var_info` が `IdentifierRef.scope_depth` を無視し、
`local_index` をそのまま `offset` として使用するため、内部ブロックスコープの変数が
関数スコープの変数とヒープアドレスを共有する。

main() の内部ブロック `{ let: i(0); ... }` の `i` が `arr[0]` と同じ `heap[LHB+0]` に配置され、
配列データが破壊される。

修正設計: [fix-block-scope-offset/](fix-block-scope-offset/)

## スコープテスト追加による失敗テスト拡大 (2026-02-17)

ブロックスコープ関連のテストが追加された結果、Bug D に起因する失敗が大幅に増加した。
現在の ws_self テスト失敗は **18件** (以前は 1件)。

### 失敗分類

| 根本原因 | 件数 | テスト |
|----------|------|--------|
| **Bug D: ブロックスコープオフセット衝突** | 11件 | 下記参照 |
| **ブロック/if/while 式の値返却未実装** | 2件 | 下記参照 |
| **Bug D + 式の値返却の複合** | 2件 | 下記参照 |
| **static 変数 + ネスト関数のスコープ** | 3件 | 下記参照 |

#### Bug D のみ (11件)

| テスト名 | 説明 |
|----------|------|
| test_example_qsort_ws_self | main 内ブロック `{ let:i }` が arr[0] と衝突 |
| test_ok_coding_c004_ws_self | if ブロック内 `let:x` が親の x と衝突 |
| test_scope_block_expr_basic_001_ws_self | ブロック内 `let:y` が親変数と衝突 |
| test_scope_block_expr_parent_scope_001_ws_self | ブロック内 `let:y` が親の x と衝突 |
| test_scope_block_var_no_collision_001_ws_self | Bug D の直接的な回帰テスト |
| test_scope_disabled_scope_block_var_001_ws_self | if ブロック内 `let:y` |
| test_scope_scope_block_001_ws_self | if ブロック内 `let:x; let:y` |
| test_scope_scope_nested_blocks_001_ws_self | 2段ネストブロック |
| test_scope_scope_shadow_multi_001_ws_self | 3段ネストでシャドーイング |
| test_scope_scope_shadowing_002_ws_self | ネストされたシャドーイング |
| test_literals_comment_japanese_001_ws_self | 日本語コメント + `if:1{ let:y }` (Bug D) |

#### ブロック/if/while 式の値返却 (2件)

`generate_block` が常に `push 0` を返すため、式の値が正しく伝播しない。

| テスト名 | 説明 |
|----------|------|
| test_control_flow_if_expr_value_001_ws_self | `x = if: 1 { 5; } else: { 10; };` |
| test_control_flow_while_expr_value_001_ws_self | `x = while: ... { i; };` |

#### Bug D + 式の値返却の複合 (2件)

| テスト名 | 説明 |
|----------|------|
| test_scope_block_expr_nested_001_ws_self | `result = { let:a; ... { let:b; a+b; }; };` |
| test_scope_block_expr_value_001_ws_self | `x = { let:a; a=3; a; };` |

#### static 変数 + ネスト関数 (3件)

ブロックスコープ変数を使用していないが AssertionFailed で失敗。
ラベル重複修正後の別のバグ（ネスト関数からの static 変数アクセスの WS コンパイル問題）。

| テスト名 | 説明 |
|----------|------|
| test_scope_scope_static_mixed_001_ws_self | static + let 混在、ネスト関数 |
| test_scope_scope_static_multi_decl_001_ws_self | 複数 static 宣言、ネスト関数 |
| test_scope_scope_static_nested_001_ws_self | ネスト関数から static アクセス |

## 関連ドキュメント

- [fix-block-scope-offset/](fix-block-scope-offset/) - Bug D 修正設計
- [fix-while-loop-stack-leak.md](../done-task/fix-while-loop-stack-leak.md) - 修正設計（Fix C）
- [remaining-ws-self-failures.md](../done-task/remaining-ws-self-failures.md) - Fix A/B の実装記録
- [whitespace-self-test-failures.md](whitespace-self-test-failures.md) - 全体の失敗テスト管理
