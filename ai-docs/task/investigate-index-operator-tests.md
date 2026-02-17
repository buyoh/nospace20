# 非配列変数へのインデックス演算子テスト失敗の調査

## 概要

`index-operator-on-non-array.md` のタスクで実装した新しいテストケースが失敗している。
テストは以下の3つ:
- `test_index_operator_non_array_001` / `test_index_operator_non_array_001_ws_self`
- `test_index_operator_non_array_002` / `test_index_operator_non_array_002_ws_self`
- `test_index_operator_non_array_003` / `test_index_operator_non_array_003_ws_self`

## 失敗の詳細

すべてのテストで以下のような失敗が発生:

```
__clog: 100
__clog: 200

thread 'test_index_operator_non_array_001' (39161528) panicked at tests/code_test.rs:219:9:
assertion `left == right` failed: stdout mismatch in test 'index_operator_non_array_001', case 'default'
Expected: "100\n200"
Actual: ""
  left: "100\n200"
 right: ""
```

- `__clog` による出力は表示されているが、実際の stdout には何も出力されていない
- 期待値は `"100\n200"` だが実際は空文字列

## 原因の仮説

### 仮説1: `__clog` の出力先が stderr

`__clog` は標準エラー出力に出力しているが、テストケースは stdout を期待している可能性。

確認方法:
- `__clog` の実装を確認
- 既存の成功しているテストで `__clog` を使用しているものを確認

### 仮説2: 非配列変数へのインデックスアクセスの実装に問題

`x[1]` が隣接変数にアクセスできていない可能性。

確認方法:
- インタプリタ・コンパイラの実装を確認
- メモリレイアウトを確認

### 仮説3: テストの期待値が間違っている

テストケースの期待値が正しくない可能性。

## 調査項目

1. `__clog` を使用している既存の成功テストを確認
2. `__clog` の実装を確認
3. 非配列変数へのインデックスアクセスの動作を手動で確認
4. メモリレイアウトを確認

## 次のステップ

調査を完了したら、テストケースまたは実装を修正する。
