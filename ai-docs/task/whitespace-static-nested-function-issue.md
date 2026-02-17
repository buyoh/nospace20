# Whitespace コンパイラでのネストされた関数の static 変数問題

作成日: 2026-02-18  
ステータス: 🔍 調査中

## 問題の概要

`test_scope_scope_static_mixed_001_ws_self` テストが失敗している。  
親関数 `test()` 内の static 変数 `shared` に、ネストされた関数 `inner()` からアクセスすると失敗する。

エラー: `AssertionFailed(0)`

## 失敗したテストケース

`resources/tests/passes/scope/scope_static_mixed_001.ns`

```nospace
# static と非 static の混在 #
func: test() {
  let: local;
  static: shared;
  local = 10;
  shared = 20;
  
  func: inner() {
    # local にはアクセス不可（非 static） #
    __assert(shared == 20);  # static なのでアクセス可能 → ここで失敗 #
    shared = 30;
  }
  
  inner();
  __assert(shared == 30);
  __assert(local == 10);  # 変更されていない #
}

func: main() {
  test();
}
```

## 現在の動作

- インタプリタモードでは正常に動作
- Whitespace コンパイラでは `inner()` 内の `__assert(shared == 20)` で失敗

## 根本原因の仮説

`compute_static_var_offsets()` 関数は、ルートスコープの `scope.functions` のみを走査している。  
ネストされた関数（関数内で定義された関数）の static 変数は考慮されていない可能性がある。

具体的には：
- `test()` の static 変数 `shared` はグローバルオフセットが計算される（関数インデックス 0）
- `inner()` はネストされた関数なので、`scope.functions` には含まれない
- `inner()` 内から `shared` にアクセスする際、関数インデックスが異なるため、正しいオフセットが取得できない可能性

## 調査項目

1. ネストされた関数の関数インデックスはどのように割り当てられているか？
2. `inner()` が `shared` にアクセスする際、`current_func_index` は何を指しているか？
3. `static_var_global_offsets` のキーは `(func_index, slot_index)` だが、ネストされた関数の場合は正しく機能するか？
4. セマンティックアナライザは、ネストされた関数からの static 変数アクセスをどう処理しているか？

## 次のステップ

1. ネストされた関数の関数インデックスを確認
2. `generate_scope()` でネストされた関数の static 変数も考慮するように修正
3. または、`get_var_info()` の static 変数判定ロジックを見直す

## 関連

- [whitespace-static-variable-issue.md](./whitespace-static-variable-issue.md) - 実装完了した static 変数サポート
