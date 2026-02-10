# Phase 5 ネスト関数実装 - スタックオーバーフロー調査

## 問題の概要

ネスト関数を含むテストを実行すると、スタックオーバーフローが発生する。

## 再現手順

```bash
cargo run --bin nospace20 -- resources/tests/passes/scope/scope_nested_func_001.ns
```

または

```bash
cargo run --bin nospace20 -- tmp/test-nested-actual.ns
```

## エラーメッセージ

```
thread 'main' (32301604) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

## テストケース

### 成功するケース（ネスト関数なし）

```nospace
# tmp/test-nested-simple.ns
func: outer() {
  __trace(1);
}

func: main() {
  __trace(0);
  outer();
}
```

結果: 成功

### 失敗するケース（ネスト関数あり）

```nospace
# tmp/test-nested-actual.ns
func: outer() {
  __trace(1);

  func: inner() {
    __trace(2);
  }

  inner();
}

func: main() {
  __trace(0);
  outer();
}
```

結果: スタックオーバーフロー

## 原因の仮説

### 仮説1: プレースホルダーの Scope が問題

パス1aで関数のプレースホルダーを作成する際、空の `Scope` を作成している。この `Scope` は `Block` に含まれ、`Block` は `Function` に含まれている。`temporary_scope` を作成する際に `identifier_map` を clone するが、その際に何らかの再帰的な参照が発生している可能性がある。

### 仮説2: analyze_internal_with_parent の無限再帰

ネスト関数の本体を解析する際に、`analyze_internal_with_parent` が再帰的に呼ばれる。その中でまた関数宣言が処理され、無限ループが発生している可能性がある。

ただし、各ネスト関数は異なる `statements` を持っているため、通常は無限ループにはならないはずである。

### 仮説3: resolver の func_map が循環参照を引き起こす

`temporary_scope` の `identifier_map` にプレースホルダーの関数が含まれており、その `identifier_map` を `resolver.enter_scope` に渡している。関数解決の際に、この `identifier_map` を参照すると、何らかの循環参照が発生している可能性がある。

## 次のステップ

1. **デバッグ出力を追加**
   - `analyze_internal_with_parent` の開始・終了時に関数名とスコープの深さを出力
   - `resolve_function` が呼ばれるたびに関数名を出力
   - プレースホルダーの作成時に関数名を出力

2. **関数宣言のホイスティング処理を簡略化**
   - プレースホルダーではなく、関数名だけを先に記録する方法を検討
   - 関数本体の解析を2パスに戻し、別の方法でホイスティングを実現

3. **段階的なテスト**
   - ネスト関数の定義だけ（呼び出しなし）で動作するか確認
   - ネスト関数の呼び出しだけで動作するか確認

## 関連ファイル

- [src/semantic_analyzer/mod.rs](../../../src/semantic_analyzer/mod.rs) - 関数宣言のホイスティング処理
- [src/semantic_analyzer/scope.rs](../../../src/semantic_analyzer/scope.rs) - Scope と ScopeResolver の定義
- [src/interpreter/exec.rs](../../../src/interpreter/exec.rs) - ユーザー定義関数の呼び出し処理

## 備考

- 簡単なテスト（ネスト関数なし）は成功しているため、基本的な関数呼び出しの仕組みは正しく動作している
- スタックオーバーフローはネスト関数を含む場合のみ発生するため、問題はネスト関数の解析または呼び出し処理にある
