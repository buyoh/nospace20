# Step 4: compiler_ws の変更

## 概要

Whitespace コンパイラで `while` のコード生成を式レベルから文レベルに移動する。

## 変更内容

### 4-1. `generate_expression` から While を削除

**ファイル**: `src/compiler_ws/expression.rs`

```rust
// 削除
ExecExpression::While(mode, cond, body) => generate_while_expression(ctx, mode, cond, body),
```

### 4-2. `generate_while_expression` 関数を削除

**ファイル**: `src/compiler_ws/expression.rs`

`generate_while_expression` 関数全体を削除。

### 4-3. `generate_statement` に While を追加

**ファイル**: `src/compiler_ws/statement.rs`

```rust
ExecStatement::While(mode, cond, body) => {
    generate_while_statement(ctx, mode, cond, body)
}
```

### 4-4. `generate_while_statement` 関数を追加

**ファイル**: `src/compiler_ws/statement.rs`

式版との差異:
- ループ終了後に `Push(0)` しない（値を返す必要がない）
- ループ本体のブロック値の discard は引き続き必要（ブロック内の式文の結果をクリーンアップ）

```rust
fn generate_while_statement(
    ctx: &mut CodeGenContext,
    mode: &ConditionMode,
    cond: &LocatedExecExpression,
    body: &Block,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    let loop_start = ctx.new_label();
    let loop_end = ctx.new_label();

    ctx.push_loop_labels(loop_start, loop_end);

    // ループ開始ラベル
    prog.push(Instruction::Label(loop_start));

    // 条件評価
    prog.append(generate_expression(ctx, cond)?);

    // ConditionMode に応じたループ終了ジャンプ
    match mode {
        ConditionMode::NonZero => {
            prog.push(Instruction::JumpIfZero(loop_end));
        }
        ConditionMode::Zero => {
            let continue_label = ctx.new_label();
            prog.push(Instruction::JumpIfZero(continue_label));
            prog.push(Instruction::Jump(loop_end));
            prog.push(Instruction::Label(continue_label));
        }
        ConditionMode::Negative => {
            let continue_label = ctx.new_label();
            prog.push(Instruction::JumpIfNegative(continue_label));
            prog.push(Instruction::Jump(loop_end));
            prog.push(Instruction::Label(continue_label));
        }
    }

    // ループ本体
    prog.append(generate_block(ctx, body)?);

    // ブロック値をクリーンアップ（generate_block は常に値をプッシュする）
    prog.push(Instruction::Discard);

    // ループ開始へジャンプ
    prog.push(Instruction::Jump(loop_start));

    // ループ終了ラベル
    prog.push(Instruction::Label(loop_end));

    ctx.pop_loop_labels();

    // 注: 式版では Push(0) していたが、文版では不要
    Ok(prog)
}
```

### 4-5. `count_nested_vars_in_expression` から While を削除

**ファイル**: `src/compiler_ws/statement.rs`

```rust
// 削除
ExecExpression::While(_mode, cond, body) => {
    count_nested_vars_in_expression(cond) + calculate_total_variable_count(body)
}
```

### 4-6. `count_nested_vars_in_statement` に While を追加

**ファイル**: `src/compiler_ws/statement.rs`

```rust
ExecStatement::While(_mode, cond, body) => {
    count_nested_vars_in_expression(cond) + calculate_total_variable_count(body)
}
```

ここで `count_nested_vars_in_expression` は条件式の内部のネスト変数をカウントし、
`calculate_total_variable_count` は本体ブロックの変数をカウントする。

## 確認ポイント

- while ループのコード生成が正しいこと
- スタックリーク（Bug C）が発生しないこと
- break / continue が正しいラベルにジャンプすること
- ループ終了後にスタックに余分な値が残らないこと
