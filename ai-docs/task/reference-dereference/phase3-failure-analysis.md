# 参照・デリファレンス Phase 3 失敗分析

## 問題

テストが一部失敗している:
- `test_operators_ref_basic_001`: 成功✓
- `test_operators_ref_deref_assign_001`: 成功✓
- `test_operators_ref_double_001`: 成功✓
- `test_operators_ref_func_arg_001`: 失敗✗
- `test_operators_ref_swap_001`: 失敗✗

失敗しているテストは共通して「関数間でアドレスを渡す」テストです。

## 根本原因

現在の実装では、`interpret_call_user_function` が新しい `LocalEnvironment` を作成します:

```rust
let mut env = LocalEnvironment::new_func(self.env, self.root_scope, &func, &arg_values);
```

この `LocalEnvironment` は独立した `scope_stack` を持ち、呼び出し元の `scope_stack` とは無関係です。これにより:

1. main() のローカル変数 x のアドレスは 0 (main の scope_stack = [main_vars])
2. そのアドレスを set_value(&x, 100) に渡す
3. set_value() 内で新しい LocalEnvironment が作成される (scope_stack = [set_value_vars])
4. `*ptr` (ptr=0) を評価すると、アドレス 0 は set_value のローカル変数を指してしまう

## 設計ドキュメントとの不一致

ai-docs/task/reference-dereference/interpreter.md では以下のように設計されている:

> アドレス空間:
>   [0 .. global_count)                         → グローバル変数
>   [global_count .. global_count + scope0_size) → スコープ0のローカル変数
>   [global_count + scope0_size .. ...)          → スコープ1のローカル変数

これは「全てのスコープが連続したアドレス空間に配置される」ことを前提としています。

しかし、現在の実装では各関数が独立した LocalEnvironment を持つため、この前提が成立していません。

## 旧実装 (C++ Whitespace コンパイラ) の参照

旧実装（`ai-docs/task/compiler/` の文書群）では、この問題を以下のように解決している:

### メモリモデル

- **全変数（グローバル・ローカル）は Whitespace のヒープ（＝単一の共有メモリ空間）に配置される**
- `LocalHeapBegin` / `LocalHeapEnd` の2つのポインタでローカル変数領域を管理
- ローカル変数のアドレス = `heap[LocalHeapBegin] + offset`（グローバルなアドレス空間上の位置）

### 関数呼び出し時のスタックフレーム管理 (convertLocalAllocate)

```
1. 現在の local_begin をスタックに退避（呼び出し元のフレーム開始位置を保存）
2. local_begin := local_end（新しいフレームは既存フレームの直後から開始）
3. local_end := local_begin + scopesize（新しいフレームの終了位置）
```

### 関数復帰時の解放 (convertLocalDeallocate)

```
1. local_end := local_begin（使用領域を元に戻す）
2. local_begin := スタックから復元（呼び出し元のフレーム開始位置を復元）
```

### 重要な設計ポイント

旧実装では関数呼び出し時に **新しいメモリ空間を作るのではなく、既存のメモリ空間を拡張する**。
これにより、呼び出し元で取得したアドレス（`&x`）は呼び出し先でもそのまま有効。

```
呼び出し前:
  ヒープ: [...globals... | ...main_vars...]
          ^GlobalPtr      ^LocalHeapBegin  ^LocalHeapEnd

呼び出し後:
  ヒープ: [...globals... | ...main_vars... | ...callee_vars...]
          ^GlobalPtr      ^old_begin        ^LocalHeapBegin    ^LocalHeapEnd
```

main の変数 x のアドレスは、関数呼び出し前後で変わらない。

## 修正案

### 方針: scope_stack を関数呼び出し間で共有する

旧実装の「既存メモリ空間を拡張する」アプローチをインタプリタに適用する。
`interpret_call_user_function` で新しい `LocalEnvironment` を作成するのではなく、
既存の `scope_stack` に新しい関数のスコープを push する。

### 修正前の動作

```
main 開始:     scope_stack = [[main_vars]]
  call f(&x):  新 LocalEnvironment: scope_stack = [[f_vars]]  ← main_vars が見えない！
    *ptr = v → get_by_address(addr) → addr が f の scope_stack にない → 不正アクセス
```

### 修正後の動作

```
main 開始:     scope_stack = [[main_vars]]
  call f(&x):  scope_stack = [[main_vars], [f_vars]]  ← main_vars も見える！
    *ptr = v → get_by_address(addr) → addr が scope_stack[0] に正しくマッピング → OK
  f 復帰:      scope_stack = [[main_vars]]
```

### 具体的なコード変更

#### `interpret_call_user_function` の修正

```rust
fn interpret_call_user_function(
    &mut self,
    id: &String,
    args: &Vec<Box<ExecExpression>>,
) -> ExpressionFlow {
    let mut arg_values = Vec::new();
    arg_values.reserve(args.len());
    for a in args {
        arg_values.push(try_expr!(self.interpret_expression(a)));
    }
    let func = self.root_scope.get_function(id.as_str()).unwrap();

    // 新しい scope を既存の scope_stack に push
    let mut variables = vec![0; func.block.scope.variable_count];
    for (i, arg_val) in arg_values.iter().enumerate() {
        if i < func.arg_indices.len() {
            variables[func.arg_indices[i]] = *arg_val;
        }
    }
    self.scope_stack.push(variables);

    // 既存の LocalEnvironment 上で関数本体を実行
    let result = match self.interpret_statements(&func.block.statements) {
        Flow::Proceed => ExpressionFlow::Value(0),
        Flow::Return(x) => ExpressionFlow::Value(x),
        Flow::Continue => panic!("internal error: unexpected continue"),
        Flow::Break => panic!("internal error: unexpected break"),
    };

    // 関数スコープを pop
    self.scope_stack.pop();
    result
}
```

### scope_depth の整合性

semantic_analyzer は各関数を独立に解析し、変数の `scope_depth` を関数内の相対位置として計算する。
`get_variable` は `scope_idx = scope_stack.len() - 1 - id.scope_depth` で動作するため、
scope_stack に呼び出し元のスコープが含まれていても問題ない:

```
scope_stack = [[main_vars], [f_vars], [block_vars]]
                                        ^depth 0
                              ^depth 1
                ^depth 2 （f 内の scope_depth は最大 1 なのでアクセスされない）
```

- depth 0 → `scope_stack[2]` = block_vars ✓
- depth 1 → `scope_stack[1]` = f_vars ✓
- depth 2 以上の IdentifierRef は semantic_analyzer が生成しないため到達しない

### Rust の借用チェッカーとの互換性

`root_scope` は `&'a Scope` 型（不変参照の Copy）であるため、
`self.root_scope.get_function(id)` の戻り値は `self` を借用しない（`'a` ライフタイムを持つ）。
したがって `func` を保持しつつ `self.scope_stack` や `self.interpret_statements()` を
呼び出すことに借用の問題は発生しない。

### `new_func` は不要にならない

`new_func` は `interpret()` のエントリポイント（main 関数の呼び出し）で引き続き使用する。
関数間呼び出し時のみ scope_stack 共有方式に切り替える。

### リスク・注意点

1. **ダングリングポインタ**: 関数ローカル変数のアドレスを return で返した場合、
   呼び出し元では pop 済みのスコープを指す。C 言語同様に未定義動作として扱う。
2. **再帰呼び出し**: 再帰時は同じ関数のスコープが scope_stack に複数積まれる。
   各呼び出しの変数は異なるスコープに存在するため正しく動作する。
3. **パフォーマンス**: scope_stack が深くなるほど `get_by_address` / `set_by_address` の
   線形探索コストが増加。将来的にはオフセットキャッシュで最適化可能。

## 実装手順

1. `interpret_call_user_function` を上記の通り修正（`new_func` 呼び出しを除去、scope push/pop に変更）
2. 既存のユニットテスト 4 件が引き続き PASS することを確認
3. 失敗している統合テスト 2 件（ref_func_arg_001, ref_swap_001）が PASS に変わることを確認
4. 全テストスイート（`cargo test`）が PASS することを確認
