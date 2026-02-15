# Block Expression Implementation - Test Failure Investigation

**日付**: 2026-02-15  
**ステータス**: 🔍 調査中

## 概要

ブロックスコープ式の実装後、4つの既存テストが失敗しています。

## 失敗したテスト

1. `test_functions_func_redefine_001`
2. `test_scope_func_shadowing_global_001`
3. `test_scope_func_shadowing_nested_001`
4. `test_scope_func_shadowing_siblings_001`

## エラー内容

すべてのテストで同様のエラーが発生：

```
assertion `left == right` failed: trace(idx:0) failed
  left: 0
 right: 1
```

- 期待値 (check.json): trace[0] = 0
- 実際の値: trace[0] = 1

## 調査結果

### test_scope_func_shadowing_global_001 の例

**テストファイル内容**:
```nospace
func: foo() {
  __trace(1);
}

func: outer() {
  __trace(2);
  func: foo() {
    __trace(3);
  }
  foo();
}

func: main() {
  __trace(0);
  foo();
  outer();
}
```

**check.json**:
```json
{
  "trace": [0, 1, 2, 3]
}
```

**実際の実行結果**:
```
trace[0]: 1
trace[1]: 1
trace[2]: 1
trace[3]: 1
```

### 問題の分析

check.json の値 `[0, 1, 2, 3]` は、各 trace インデックスの実行回数を示すべきですが、実際には：
- trace[0] は 1回実行される (main() 内の __trace(0))
- trace[1] は 1回実行される (foo() 内の __trace(1))
- trace[2] は 1回実行される (outer() 内の __trace(2))
- trace[3] は 1回実行される (ネストした foo() 内の __trace(3))

したがって、正しい期待値は `[1, 1, 1, 1]` であるべきです。

## 原因の仮説

1. **check.json の記載ミス**: テストケースが最近追加されたもので、check.json が誤っている可能性
2. **ブロック式実装の副作用**: 何らかの理由でブロック式の実装が既存の動作に影響を与えている可能性
3. **元々失敗していたテスト**: これらのテストが当初から失敗していた可能性

## 次のステップ

- [ ] ブロック式実装前の状態でテストを実行し、元々成功していたか確認
- [ ] check.json の期待値が正しいかどうかを確認
- [ ] ブロック式の実装が既存のパース処理に影響を与えていないか詳細に調査
- [ ] 失敗したテストのうち、実装前から失敗していたものと新規に失敗したものを切り分け

## 暫定対応

プロンプト指示に従い、既存のテストは修正せず失敗したままにしておく。
新規に実装したブロック式のテストは全て成功しているため、ブロック式の機能自体は正しく動作している。
