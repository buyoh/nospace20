# compiler_ws の変更設計

## 変更方針

compiler_ws（Whitespace コンパイラ）は、void 型の式がスタックに値を残さないようにコード生成を最適化する。semantic_analyzer で型チェック済みのため、void 式の値が使用されるケースは存在しない。

## 変更対象ファイル

- `src/compiler_ws/expression.rs` — 式のコード生成
- `src/compiler_ws/statement.rs` — 文のコード生成

## 1. expression.rs の変更

### generate_while_expression

**現状**:
```
Label(loop_start)
条件評価
JumpIfZero(loop_end)
generate_block(body)
Discard               ← ブロック値を破棄
Jump(loop_start)
Label(loop_end)
Push(0)               ← while 式の値として 0
```

**変更後**:
```
Label(loop_start)
条件評価
JumpIfZero(loop_end)
generate_block_void(body)   ← void ブロックとして生成（値を残さない）
Jump(loop_start)
Label(loop_end)
                              ← Push(0) を削除（void なので値不要）
```

- while 式は void 型。値をスタックに残さない。
- `generate_block_void` はブロック内の全式を文として処理（最後の式も Discard）。

### generate_if_expression

**現状**:
```
条件評価
JumpIfZero(else_label)
generate_block(then_block)   ← 値がスタックに残る
Jump(end_label)
Label(else_label)
generate_block(else_block)   ← 値がスタックに残る
Label(end_label)
```

**変更後（int 型 if）**: 変更なし

**変更後（void 型 if）**:
```
条件評価
JumpIfZero(else_label)
generate_block_void(then_block)   ← 値を残さない
Jump(end_label)
Label(else_label)
generate_block_void(else_block)   ← 値を残さない
Label(end_label)
                                    ← 値をスタックに残さない
```

型判定は `ExecExpression::If` の型推論結果を使って分岐する。

### UserFunction 呼び出し

**現状**: `Call(func_label)` 後にスタックに値が1つ残る前提。

**変更後（int 関数）**: 変更なし

**変更後（void 関数）**: `Call(func_label)` 後にスタックに値が残らない。

## 2. statement.rs の変更

### generate_statement の式文処理

**現状**:
```rust
ExecStatement::Expression(expr) => {
    let mut prog = generate_expression(ctx, expr)?;
    prog.push(Instruction::Discard);  // 常に Discard
    Ok(prog)
}
```

**変更後**:
```rust
ExecStatement::Expression(expr) => {
    let expr_type = expr.infer_type(ctx.functions());
    let mut prog = generate_expression(ctx, expr)?;
    if expr_type == ValueType::Int {
        prog.push(Instruction::Discard);  // int の場合のみ Discard
    }
    Ok(prog)
}
```

### generate_block

**現状**: 最後の式文のみ Discard しない（値をスタックに残す）。空ブロックは `Push(0)`。

**変更後**: ブロックの型が void か int かに応じて分岐。

```rust
fn generate_block(ctx, block, as_void: bool) -> Result<...> {
    if as_void {
        // 全ての式文に Discard を適用（値を残さない）
        for stmt in &block.statements {
            prog.extend(generate_statement(ctx, stmt)?);
        }
    } else {
        // 現状維持: 最後の式文のみ値を残す
        // 空ブロックは Push(0)
    }
}
```

あるいは `generate_block_void` と `generate_block_value` の2関数に分ける。

### 関数定義のデフォルト return

**現状**: 関数末尾に `deallocate` + `Push(0)` + `Return`

**変更後（void 関数）**: `deallocate` + `Return`（Push(0) を省略）

**変更後（int 関数）**: 変更なし

## 3. 型情報の取得方法

compiler_ws が式の型を知るには、`ExecExpression::infer_type()` メソッドを呼び出す。このメソッドはグローバル関数テーブル (`&[Function]`) を引数に取る。

`CodeGenContext` にグローバル関数テーブルへの参照を追加する:

```rust
impl CodeGenContext {
    pub fn functions(&self) -> &[Function] {
        // グローバル関数テーブルを返す
    }
}
```

## 4. 影響範囲まとめ

| 関数 | 変更内容 |
|------|----------|
| `generate_while_expression` | Push(0) 削除、本体を void ブロックとして生成 |
| `generate_if_expression` | void if の場合、void ブロックとして生成 |
| `generate_block` | void/int モードの分岐 |
| `generate_statement` (式文) | void 式は Discard 省略 |
| 関数定義のデフォルト return | void 関数は Push(0) 省略 |
| `CodeGenContext` | functions 参照の追加 |
