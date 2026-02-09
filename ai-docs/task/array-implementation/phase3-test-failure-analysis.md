# Phase 3 実装後のテスト失敗調査

## 実施日

2026年2月10日

## 実装内容

Phase 3 (インタプリタ) の配列アクセス機能を実装しました:

- `ExecExpression::ArrayAccess` の評価（読み取り）
- 配列要素への代入処理
- `&arr[i]` の参照取得
- 境界チェック

## テスト結果

### 成功した機能

- 配列の宣言
- 配列初期化 (`let: arr[3](10, 20, 30)`)
- 配列の直接アクセス (非ループ)
- 配列の `arr` 単体アクセス (`arr` は `arr[0]` と同義)

### 失敗したテスト

1. **test_array_basic**: while ループ内での配列アクセスが動作しない
   - 期待: `arr4[0]=0, arr4[2]=20, arr4[4]=40`
   - 実際: `arr4[0]=0, arr4[2]=0, arr4[4]=0`
   - ループ内で `arr4[i] = i * 10` が実行されているはずだが、値が設定されない

2. **test_array_reference**: 配列要素の参照操作で不正な値
   - 期待: `arr2[2]=6`
   - 実際: `arr2[2]=3`
   - ループ内で `*(ptr + i) = *(ptr + i) * 2` が実行されているはずだが、最後の要素だけ2倍にならない

3. **test_array_static**: static 配列の動作確認が必要

## デバッグログ

### test_array_basic の直接実行

```bash
$ cargo run --bin nospace20 -- resources/tests/passes/array-basic.ns
10
20
30
100
200
300
42
42
0
0
0
```

while ループの後の配列要素が全て 0 のまま。

## 仮説

### 仮説1: while ループ内でのスコープ問題

while ループ内で宣言された配列が、ループ外の配列とは異なるスロットを参照している可能性。

### 仮説2: 配列アクセスのインデックス計算の問題

変数 `i` の値が正しく取得されていないか、オフセット計算が誤っている可能性。

### 仮説3: 代入処理の問題

`arr[i] = val` の代入が、実際には配列の要素ではなく別の場所に書き込んでいる可能性。

## 次のステップ

1. 単純な配列アクセステストを作成して切り分け
2. インタプリタのデバッグログを追加
3. while ループなしで配列アクセスが動作するか確認
4. インデックスが変数の場合と定数の場合で動作を比較

## 関連ファイル

- [src/interpreter/exec.rs](../../src/interpreter/exec.rs)
- [resources/tests/passes/array-basic.ns](../../resources/tests/passes/array-basic.ns)
- [resources/tests/passes/array-reference.ns](../../resources/tests/passes/array-reference.ns)
- [resources/tests/passes/array-static.ns](../../resources/tests/passes/array-static.ns)
