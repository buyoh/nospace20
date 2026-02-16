# whitespace-self テスト失敗調査

## 概要

`whitespace-self` モード（独自 WhitespaceVM で nospace コンパイル結果を実行）を追加した際に、15件のテストが失敗した。
これらは新規テストであり、既存テストへの影響はない。

## 根本原因

**Whitespace コンパイラ (`compiler_ws`) のラベルアロケータにおけるラベル ID 重複バグ**

### 問題のメカニズム

`CodeGenContext::enter_function()` でラベルアロケータを `clone()` している。
関数本体内で割り当てたラベル（if/else, while 等の制御構造用）は、クローンされた子コンテキストにのみ反映され、親コンテキストの `next_id` には反映されない。

**該当コード**: `src/compiler_ws/context.rs` の `enter_function()`

```rust
pub fn enter_function(&self, local_var_count: usize) -> CodeGenContext<'a> {
    CodeGenContext {
        scope: self.scope,
        labels: self.labels.clone(),  // ← ここが問題
        // ...
    }
}
```

### 具体例: `e0-00-puts`

```
// puts() 関数定義:
//   関数ラベル label_16, label_17 を親コンテキストから割り当て (next_id = 18)
//   enter_function() でクローン → 子コンテキスト next_id = 18
//   while ループ内で label_18 (ループ開始), label_19 (ループ終了) を子コンテキストから割り当て
//   子コンテキスト破棄 → 親コンテキストの next_id は 18 のまま
//
// main() 関数定義:
//   関数ラベル label_18, label_19 を親コンテキストから割り当て ← 重複！
```

生成されたコード上のラベル定義 (実際の `--target mnemonic` 出力で確認済み):
```
label_18   ← puts() 内の while ループ開始 (1回目の定義)
label_19   ← puts() 内の while ループ終了 (1回目の定義)
label_18   ← main() のエントリポイント (2回目の定義、HashMap で上書き)
label_19   ← main() のスキップ先 (2回目の定義、HashMap で上書き)
```

WhitespaceVM の `collect_labels()` は HashMap を使用するため、後に定義されたラベルが優先される。
結果として puts() 内の `jmp label_18` (ループバック) が main() のエントリに飛び、無限ループが発生する。

### 副次的問題: `generate_return` での clone

`statement.rs` の `generate_return()` でも `ctx.clone()` が発生している。
return 式内に if/while 式が含まれる場合に同様のラベル重複が起きる可能性がある。

```rust
fn generate_return(ctx: &CodeGenContext, expr: &...) -> Result<...> {
    prog.append(expression::generate_expression(&mut ctx.clone(), expr)?);
    //                                               ^^^^^^^^^^^ clone
}
```

## 失敗パターンの分類

### 発生条件

ラベル重複は以下の **両方** の条件を満たす場合にのみ発生する:

1. **複数の関数が定義されている**（最初の関数定義で子コンテキストにラベルが割り当てられ、次の関数定義でラベル ID が重複）
2. **関数本体内に if/else または while がある**（制御構造がラベルを割り当てる）

成功テスト（99件）は以下のいずれか:
- 関数定義が 1 つのみ → ラベル競合なし
- 複数関数があるが if/while/else を関数本体で使っていない → 子コンテキストでラベル未割当

### パターン1: ステップ数上限超過 (Suspended)

関数内の while ループのラベルが後続関数のラベルと重複し、ループバックが別の関数に飛ぶことで無限ループが発生。

| テスト名 | パス | 原因詳細 |
|---|---|---|
| test_example_puts_ws_self | examples/e0-00-puts | puts() 内 while のラベルが main() と重複 |
| test_legacy_015_ws_self | legacy/legacy_015 | fibo() 内 if のラベルが main() と重複 |
| test_scope_func_shadowing_nested_001_ws_self | scope/func_shadowing_nested_001 | ネスト関数内のラベル重複 |
| test_scope_func_shadowing_siblings_001_ws_self | scope/func_shadowing_siblings_001 | 同上 |
| test_scope_scope_nested_func_001_ws_self | scope/scope_nested_func_001 | 同上 |
| test_scope_scope_static_counter_factory_001_ws_self | scope/scope_static_counter_factory_001 | 同上 |
| test_scope_scope_static_mixed_001_ws_self | scope/scope_static_mixed_001 | 同上 |
| test_scope_scope_static_multi_decl_001_ws_self | scope/scope_static_multi_decl_001 | 同上 |
| test_scope_scope_static_nested_001_ws_self | scope/scope_static_nested_001 | 同上 |

### パターン2: 出力不一致

関数内の if/else 分岐ラベルが後続関数のラベルと重複し、条件分岐先が誤っている。

| テスト名 | パス | 原因詳細 |
|---|---|---|
| test_example_fibonacci_ws_self | examples/e0-01-fibonacci | fibo() 内 if/else のラベルが main() と重複 |
| test_example_qsort_ws_self | examples/e1-00-qsort | 複数関数での大規模ラベル重複 |
| test_legacy_011_ws_self | legacy/legacy_011 | 関数内分岐のラベル重複 |
| test_legacy_012_ws_self | legacy/legacy_012 | test() 内 6個の if のラベルが main() と重複 |
| test_legacy_014_ws_self | legacy/legacy_014 | 関数内分岐のラベル重複 |
| test_legacy_020_ws_self | legacy/legacy_020 | check() 内論理演算+分岐のラベル重複 |

## 修正方針

### アプローチ: ラベルアロケータのカウンタ同期

関数コード生成完了後に、子コンテキストのラベルカウンタ (`next_id`) を親コンテキストに同期させる。

#### 修正箇所

1. **`src/compiler_ws/label.rs`**: `LabelAllocator` に同期メソッドを追加
   ```rust
   impl LabelAllocator {
       /// 子アロケータの next_id が自身より大きい場合に同期
       pub fn sync_next_id(&mut self, other: &LabelAllocator) {
           if other.next_id > self.next_id {
               self.next_id = other.next_id;
           }
       }
   }
   ```

2. **`src/compiler_ws/context.rs`**: カウンタ同期メソッドを追加
   ```rust
   impl CodeGenContext {
       /// 子コンテキストで消費されたラベルカウンタを同期
       pub fn sync_labels_from(&mut self, child: &CodeGenContext) {
           self.labels.sync_next_id(&child.labels);
       }
   }
   ```

3. **`src/compiler_ws/statement.rs`**: `generate_function_definition()` で同期呼び出しを追加
   ```rust
   fn generate_function_definition(ctx: &mut CodeGenContext, ...) {
       // ...
       let mut local_ctx = ctx.enter_function(local_var_count);
       // ... 関数本体コード生成 ...
       ctx.sync_labels_from(&local_ctx);  // ← 追加
       // ...
   }
   ```

4. **`src/compiler_ws/statement.rs`**: `generate_return()` のシグネチャを `&mut` に変更
   ```rust
   fn generate_return(
       ctx: &mut CodeGenContext,  // &CodeGenContext → &mut に変更
       expr: &ExecExpression,
   ) -> Result<WsProgram, CompileError> {
       let mut prog = WsProgram::new();
       prog.append(expression::generate_expression(ctx, expr)?);  // clone 不要に
       // ...
   }
   ```

### 代替アプローチ（参考）

- `Rc<RefCell<LabelAllocator>>` で共有所有: 確実だがランタイムオーバーヘッドと複雑性が増す
- ラベルアロケータを参照で渡す: 大規模リファクタリングが必要

### テスト計画

1. 修正後、全15件の失敗テストがパスすることを確認
2. 既存99件の成功テストが引き続きパスすることを確認
3. 他モード（interpreter, whitespace）のテストに影響がないことを確認

## ステータス

調査完了 - 修正作業を開始可能
