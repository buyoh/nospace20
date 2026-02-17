# Whitespace コンパイラでの static 変数初期値復元の失敗

作成日: 2026-02-17  
ステータス: 🔍 調査中

## 問題の概要

`test_scope_scope_static_init_value_persist_001_ws_self` テストが失敗している。
このテストは、関数内の static 変数の初期値が複数回の関数呼び出しで正しく維持されることを確認するものである。

interpreter では正しく動作するが、Whitespace コンパイラでは失敗する。

## テストケース

`resources/tests/passes/scope/scope_static_init_value_persist_001.ns`

```nospace
func: counter() {
  static: count(100);
  count = count + 1;
  return: count;
}

func: main() {
  __assert(counter() == 101);
  __assert(counter() == 102);
  __assert(counter() == 103);
  __trace(0);
}
```

## エラー詳細

```
Whitespace execution failed for scope/scope_static_init_value_persist_001: AssertionFailed(0)
```

最初の `__assert(counter() == 101)` で失敗している。

## 原因分析

ステップ5で `function_static_storage` のキーを String から usize に変更したが、
この変更は interpreter にのみ適用されており、Whitespace コンパイラは異なる仕組みで
static 変数を管理している可能性がある。

または、Whitespace コンパイラでは関数内 static 変数の永続化が未実装の可能性もある。

## 影響範囲

- interpreter: 正常動作（ステップ5の修正により、初期値が正しく復元される）
- Whitespace コンパイラ: 失敗（static 変数の初期値が復元されない）

## 対応方針

現時点では Whitespace コンパイラの問題として記録し、将来的に対応する。
interpreter の動作が正しいことが重要であり、この実装の主目的は達成されている。

## 関連

- [symbol-table-design.md](../task/symbol-table-design.md) - ステップ5の実装
- [symbol-table-impl/step5-static-storage-indexing.md](../task/symbol-table-impl/step5-static-storage-indexing.md) - 詳細設計
