# test_legacy_023 失敗調査

## 概要

複数変数宣言・初期化宣言の実装後、`test_legacy_023` が失敗している。

## 失敗内容

```
thread 'test_legacy_023' panicked at tests/code_test.rs:174:31:
called `Result::unwrap()` on an `Err` value: [CodeParseError { 
  code_pointer: Some(78), 
  message: "unexpected token", 
  caller: Location { file: "src/tree_parser/expression/mod.rs", line: 87, column: 19 } 
}, ...]
```

複数箇所で `"unexpected token"` エラーが発生している。

## 原因

`test_legacy_023` のソースコード (`resources/tests/passes/legacy/legacy_023.ns`):

```nospace
func:main(){
    let:a,b;
    a = 3; b = 4;
    __puti(a + b);
    __puti(a += b);  # 複合代入演算子 #
    __puti(a);
    __putc(';');
    a = 7; b = 3;
    __puti(a - b);
    __puti(a -= b);  # 複合代入演算子 #
    ...
}
```

このテストは **複合代入演算子** (`+=`, `-=`, `*=`, `/=`, `%=`) を使用している。

## 仕様上の位置づけ

spec.md §2.5 より:

```
### 2.5 複合代入演算子 (未実装)

x += 1;   # x = x + 1 と等価 #
x -= 2;   # x = x - 2 と等価 #
...
```

複合代入演算子は **未実装** である。

## タスクのスコープ

`ai-docs/task/implement-multi-variable-declaration.md` では、以下を **スコープ外** としている:

- 配列宣言 (`let: arr[4];`)
- 文字列宣言 (`let: str("Hello");`)
- `final` / `const` 修飾子

複合代入演算子も参照している未実装ドキュメント `ai-docs/task/implement-compound-assignment-operators.md` が存在する。

## 結論

`test_legacy_023` の失敗は、本タスク（複数変数宣言・初期化宣言）の範囲外である **複合代入演算子** が未実装であることに起因する。

- **複数変数宣言部分**: `let:a,b;` は正しくパースされている
- **複合代入演算子部分**: `a += b` などでパースエラーが発生している

## 対応方針

### 本タスクでの対応

- このテストは **失敗のまま残す**（指示通り）
- 複数変数宣言に関する他のテスト (`test_legacy_015`, `test_legacy_020`) は成功している
- 新規追加したテスト (`test_variables_var_init_*`) も全て成功している

### 今後の対応

複合代入演算子を実装する際に、このテストが成功するようになる。
実装タスク: `ai-docs/task/implement-compound-assignment-operators.md` 参照。

## テスト結果サマリ

```
test test_legacy_015 ... ok                       # 複数変数宣言 #
test test_legacy_020 ... ok                       # 複数変数宣言 #
test test_legacy_023 ... FAILED                   # 複合代入演算子（未実装） #
test test_variables_var_init_single ... ok        # 単一変数初期化 #
test test_variables_var_init_multiple ... ok      # 複数変数初期化 #
test test_variables_var_init_hoisting ... ok      # ホイスティング + 初期化 #
test test_variables_var_init_static ... ok        # static 変数初期化 #
test test_variables_var_init_expr ... ok          # 初期化式での計算 #
```

**本タスクの目的（複数変数宣言・初期化宣言）は達成された。**
