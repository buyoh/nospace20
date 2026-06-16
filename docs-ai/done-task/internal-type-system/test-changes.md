# テストの修正・追加計画

## 1. 修正が必要な既存テスト

### while_expr_value_001.ns

**ファイル**: `resources/tests/passes/control_flow/while_expr_value_001.ns`

**現状**: while 式の戻り値が常に 0 であることをテスト
```
x = while: i - 3 { ... };   # → x == 0 をアサート
x = while: 0 { 42; };       # → x == 0 をアサート
x = while: 1 { break; };    # → x == 0 をアサート
```

**変更**: while が void になるため、`x = while: ...` はコンパイルエラーになる。このテストは**削除**し、代わりに:
- while 式を式文として正しく使えることを確認するテストを用意（既存の他の while テストで充足）
- while の戻り値を代入しようとしたらエラーになるテストを `compile_error` に追加

### if_expr_value_001.ns

**ファイル**: `resources/tests/passes/control_flow/if_expr_value_001.ns`

**現状**: if 式の戻り値をテスト。L47-52 で else なし if のテストを含む。

**変更**:
- L1-44（else 付き if の戻り値テスト）→ **変更なし**（else 付き if で両方 int なら int）
- L47-48（`x = if: 1 { 77; };`）→ **削除**（else なし if は void、代入不可）
- L51-52（`x = if: 0 { 77; };`）→ **削除**（同上）

修正後のテスト内容:
```
# if 式の戻り値 #
func: main() {
  __trace(0);
  let:x;

  # else 付き if 式は then/else の値を返す #
  x = if: 1 { 5; } else: { 10; };
  __assert(x == 5);

  x = if: 0 { 5; } else: { 10; };
  __assert(x == 10);

  # 式の値を直接使用 #
  __assert(if: 1 { 42; } else: { 0; } == 42);
  __assert(if: 0 { 0; } else: { 99; } == 99);

  # 計算式の結果を返す #
  x = if: 1 { 2 + 3; } else: { 7 * 8; };
  __assert(x == 5);

  # ネストした if #
  x = if: 1 {
    if: 0 { 10; } else: { 20; };
  } else: { 30; };
  __assert(x == 20);

  # else なし if は void（式文としては使える） #
  if: 1 { __trace(0); };
}
```

### block_expr_empty_001.ns

**ファイル**: `resources/tests/passes/scope/block_expr_empty_001.ns`

**現状**: 空ブロック `{}` の値が 0 であることをテスト
```
y = {};
__assert(y == 0);
```

**変更**: 空ブロックは void なので `y = {}` はコンパイルエラーになる。

修正後のテスト内容:
```
# Empty block expression (void) #
func: main() {
  __trace(0);
  # 空ブロックは void だが式文としては使える #
  {};
  __trace(0);
}
```

## 2. 追加する新規テスト（コンパイルエラー: compile_error）

### void_while_assign.ns

**目的**: while の戻り値を変数に代入しようとするとエラー

```
func: main() {
  let: x;
  let: i(3);
  x = while: i { i = i - 1; };
}
```

check.json: `{ "compile_error": true }`

### void_if_no_else_assign.ns

**目的**: else なし if の戻り値を変数に代入しようとするとエラー

```
func: main() {
  let: x;
  x = if: 1 { 5; };
}
```

check.json: `{ "compile_error": true }`

### void_func_assign.ns

**目的**: void 関数の戻り値を変数に代入しようとするとエラー

```
func: voidfn() {
  let: x(1);
}

func: main() {
  let: x;
  x = voidfn();
}
```

check.json: `{ "compile_error": true }`

### void_in_operation.ns

**目的**: void 式を演算に使おうとするとエラー

```
func: voidfn() {
  let: x(1);
}

func: main() {
  let: x;
  x = voidfn() + 1;
}
```

check.json: `{ "compile_error": true }`

### void_in_condition.ns

**目的**: void 式を条件式に使おうとするとエラー

```
func: voidfn() {
  let: x(1);
}

func: main() {
  while: voidfn() { };
}
```

check.json: `{ "compile_error": true }`

### void_func_mixed_return.ns

**目的**: return ありなしが混在する関数はエラー

```
func: mixed(x) {
  if: x {
    return: 1;
  };
}

func: main() {
  mixed(1);
}
```

check.json: `{ "compile_error": true }`

### void_if_mixed_branches.ns

**目的**: if/else で片方が void、もう片方が int のとき、全体が void になることをテスト

```
func: main() {
  let: x;
  # while を含む else ブロックは void なので、全体が void #
  # x = if: 1 { 5; } else: { while: 0 {}; }; ← これはエラー #

  # 式文としては OK #
  if: 1 { 5; } else: { while: 0 {}; };
  __trace(0);
}
```

check.json: `{ "trace_hit_counts": [1] }`

### void_if_mixed_assign.ns

**目的**: if/else の片方が void のとき代入するとエラー

```
func: main() {
  let: x;
  x = if: 1 { 5; } else: { while: 0 {}; };
}
```

check.json: `{ "compile_error": true }`

## 3. 追加する新規テスト（正常系: passes）

### void_expr_statement.ns

**目的**: void 式を式文として使える（値を使わない）

```
func: voidfn() {
  let: x(1);
}

func: main() {
  __trace(0);
  voidfn();
  while: 0 {};
  if: 1 { 1; };
  {};
  __trace(0);
}
```

check.json: `{ "trace_hit_counts": [2] }`

### void_func_return_type.ns

**目的**: void 関数と int 関数の型が正しく推論される

```
func: intfn() {
  return: 42;
}

func: voidfn() {
  let: x(1);
}

func: main() {
  __trace(0);
  let: x;
  x = intfn();
  __assert(x == 42);
  voidfn();
  __trace(0);
}
```

check.json: `{ "trace_hit_counts": [2] }`

### void_if_else_int.ns

**目的**: if/else の両ブロックが int なら全体が int

```
func: main() {
  __trace(0);
  let: x;
  x = if: 1 { 10; } else: { 20; };
  __assert(x == 10);
  x = if: 0 { 10; } else: { 20; };
  __assert(x == 20);
  __trace(0);
}
```

check.json: `{ "trace_hit_counts": [2] }`

## 4. テスト配置

| テスト | ディレクトリ |
|--------|-------------|
| void_while_assign.ns | `resources/tests/compile_errors/type_system/` |
| void_if_no_else_assign.ns | `resources/tests/compile_errors/type_system/` |
| void_func_assign.ns | `resources/tests/compile_errors/type_system/` |
| void_in_operation.ns | `resources/tests/compile_errors/type_system/` |
| void_in_condition.ns | `resources/tests/compile_errors/type_system/` |
| void_func_mixed_return.ns | `resources/tests/compile_errors/type_system/` |
| void_if_mixed_assign.ns | `resources/tests/compile_errors/type_system/` |
| void_expr_statement.ns | `resources/tests/passes/type_system/` |
| void_func_return_type.ns | `resources/tests/passes/type_system/` |
| void_if_else_int.ns | `resources/tests/passes/type_system/` |
| void_if_mixed_branches.ns | `resources/tests/passes/type_system/` |

## 5. 影響を受けないテストの確認

以下のカテゴリのテストは影響を受けない:
- while/if を式文としてのみ使用しているテスト（大多数）
- else 付き if の戻り値を使用しているテスト（両方 int なら OK）
- 関数の return 値を正しく使用しているテスト
- ブロック式で最後に式文を持つテスト（int のまま）
