# Step 3: compiler_ws / interpreter の対応

## 概要

`LocatedExecExpression` の導入に伴い、`compiler_ws` および `interpreter` の式処理を更新する。
`compiler_ws` では式ごとに `ctx.set_location()` を呼ぶことで、エラー報告の位置精度が式レベルに向上する。

## compiler_ws の変更

### expression.rs: generate_expression

引数の型を `&ExecExpression` → `&LocatedExecExpression` に変更し、
先頭で `ctx.set_location()` を呼ぶ:

```rust
// Before
pub fn generate_expression(
    ctx: &mut CodeGenContext,
    expr: &ExecExpression,
) -> Result<WsProgram, CompileError>

// After
pub fn generate_expression(
    ctx: &mut CodeGenContext,
    located_expr: &LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    ctx.set_location(&located_expr.location);
    let expr = &located_expr.expression;
    match expr {
        // ... (既存ロジックはそのまま)
    }
}
```

これにより、式レベルのエラー（`make_error` 経由）が式の正確な位置を報告する。

### make_error の変更

Phase 2 では `make_error` はそのまま利用可能。`ctx.current_location()` が式の位置を返すようになる。

### 内部ヘルパー関数の引数変更

`ExecExpression` を直接受け取るヘルパーは、`LocatedExecExpression` の内部の `ExecExpression` を参照する形にする。
ただし一部のヘルパーは再帰的に `generate_expression` を呼ぶため、`LocatedExecExpression` を受け取る必要がある:

| 関数 | 引数変更 | 理由 |
|------|---------|------|
| `generate_expression` | `&LocatedExecExpression` | エントリポイント、位置設定 |
| `generate_load_variable` | `&IdentifierRef` | 変更なし |
| `generate_variable_address` | `&IdentifierRef` | 変更なし |
| `generate_array_element_address` | `index_expr: &LocatedExecExpression` | `generate_expression` を再帰呼び出し |
| `generate_array_access` | `index_expr: &LocatedExecExpression` | 同上 |
| `generate_unary_op` | `inner: &LocatedExecExpression` | `generate_expression` を再帰呼び出し、かつ `inner.expression` のパターンマッチあり |
| `generate_binary_op` | `left/right: &LocatedExecExpression` | `generate_expression` を再帰呼び出し、かつ `left.expression` のパターンマッチあり |
| `generate_store_variable` | `value_expr: &LocatedExecExpression` | `generate_expression` 再帰呼び出し |
| `generate_store_array` | `index_expr/value_expr: &LocatedExecExpression` | 同上 |
| `generate_function_call` | `args: &[Box<LocatedExecExpression>]` | `generate_expression` 再帰呼び出し |
| `generate_builtin_*` | `args: &[Box<LocatedExecExpression>]` | 同上 |
| `generate_if_expression` | `cond: &LocatedExecExpression` | `generate_expression` 再帰呼び出し |
| `generate_while_expression` | `cond: &LocatedExecExpression` | 同上 |
| `generate_return` | `expr: &LocatedExecExpression` | 同上 |

### 代入式の左辺パターンマッチ

`generate_binary_op` 内の `Assign` ケースでは左辺の `ExecExpression` のパターンマッチが必要:

```rust
// Before
Operator2::Assign => {
    match left {
        ExecExpression::Variable(var_ref) => { ... }
        ExecExpression::ArrayAccess(var_ref, index_expr, _) => { ... }
        ExecExpression::Operation1(Operator1::Deref, addr_expr) => { ... }
        _ => { return Err(make_error(ctx, "...")); }
    }
}

// After
Operator2::Assign => {
    match &left.expression {  // .expression を経由
        ExecExpression::Variable(var_ref) => { ... }
        ExecExpression::ArrayAccess(var_ref, index_expr, _) => { ... }
        ExecExpression::Operation1(Operator1::Deref, addr_expr) => { ... }
        _ => { return Err(make_error(ctx, "...")); }
    }
}
```

### 単項演算の Ref のパターンマッチ

同様に `generate_unary_op` 内:

```rust
// Before
Operator1::Ref => {
    match inner {
        ExecExpression::Variable(var_ref) => { ... }
        ExecExpression::ArrayAccess(var_ref, index_expr, _) => { ... }
        _ => { return Err(make_error(ctx, "...")); }
    }
}

// After
Operator1::Ref => {
    match &inner.expression {  // .expression を経由
        ExecExpression::Variable(var_ref) => { ... }
        ExecExpression::ArrayAccess(var_ref, index_expr, _) => { ... }
        _ => { return Err(make_error(ctx, "...")); }
    }
}
```

### statement.rs の変更

`ExecStatement` 内の `Box<ExecExpression>` → `Box<LocatedExecExpression>` の型変更に伴い:

```rust
// generate_statement 内
ExecStatement::Expression(located_expr) => {
    let mut prog = expression::generate_expression(ctx, located_expr)?;
    prog.push(Instruction::Discard);
    Ok(prog)
}
ExecStatement::Return(Some(located_expr)) => {
    generate_return(ctx, located_expr)
}
```

`generate_block` 内の最後の式の処理:

```rust
// Before
ExecStatement::Expression(expr) => {
    prog.append(expression::generate_expression(ctx, expr)?);
}

// After
ExecStatement::Expression(located_expr) => {
    prog.append(expression::generate_expression(ctx, located_expr)?);
}
```

`count_nested_vars_in_expression` / `count_nested_vars_in_statement` :
これらは `ExecExpression` を再帰的に探索する。`LocatedExecExpression` 経由のアクセスに変更:

```rust
fn count_nested_vars_in_expression(located_expr: &LocatedExecExpression) -> usize {
    match &located_expr.expression {
        ExecExpression::If(cond, then_block, else_block) => {
            count_nested_vars_in_expression(cond) + ...
        }
        // ...
    }
}

fn count_nested_vars_in_statement(stmt: &ExecStatement) -> usize {
    match stmt {
        ExecStatement::Expression(located_expr) | ExecStatement::Return(Some(located_expr)) => {
            count_nested_vars_in_expression(located_expr)
        }
        // ...
    }
}
```

## interpreter の変更

### exec.rs: interpret_expression

引数の型を `&Box<ExecExpression>` → `&Box<LocatedExecExpression>` に変更:

```rust
// Before
fn interpret_expression(&mut self, expr: &Box<ExecExpression>) -> ExpressionFlow {
    match expr.as_ref() {
        ExecExpression::Operation1(op, expr1) => ...

// After
fn interpret_expression(&mut self, located_expr: &Box<LocatedExecExpression>) -> ExpressionFlow {
    match &located_expr.expression {
        ExecExpression::Operation1(op, expr1) => ...
```

### 内部ヘルパーの引数変更

`interpret_operation1`, `interpret_operation2`, `interpret_while`, `interpret_if`, `interpret_call_function` 等の引数を同様に更新。

パターンマッチで `ExecExpression` を参照する箇所は `.expression` 経由に変更:

```rust
// interpret_operation2 内の代入処理
fn interpret_operation2(
    &mut self,
    op: &Operator2,
    expr1: &Box<LocatedExecExpression>,
    expr2: &Box<LocatedExecExpression>,
) -> ExpressionFlow {
    match op {
        Operator2::Assign => {
            match &expr1.expression {  // .expression を経由
                ExecExpression::Variable(id_ref) => { ... }
                ExecExpression::ArrayAccess(id_ref, index_expr, _) => { ... }
                // ...
            }
        }
        // ...
    }
}
```

### interpret_block_statements / interpret_exec_statements

これらは `LocatedExecStatement` を処理するが、内部の `ExecStatement::Expression` が
`Box<LocatedExecExpression>` を持つようになるため、呼び出し先が変わる:

```rust
ExecStatement::Expression(located_expr) => {
    self.interpret_expression(located_expr)
}
```

## 変更対象ファイル一覧

| ファイル | 変更内容 | 影響度 |
|---------|---------|-------|
| `src/compiler_ws/expression.rs` | `generate_expression` + 全ヘルパー引数型変更、パターンマッチ更新 | 大 |
| `src/compiler_ws/statement.rs` | `ExecStatement` 参照の型更新、`count_nested_vars_*` 更新 | 中 |
| `src/interpreter/exec.rs` | `interpret_expression` + 全ヘルパー引数型変更、パターンマッチ更新 | 大 |

## Phase 2 完了後の効果

### Before (Phase 1)

```
Compile error at line 5, column 1: Reference operator (&) can only be applied to variables
```

→ 行5 は式を含む文の開始位置であり、`&` 演算子の実際の位置からはずれることがある

### After (Phase 2)

```
Compile error at line 5, column 10: Reference operator (&) can only be applied to variables
```

→ 行5 列10 は `&(expr)` という式の正確な開始位置

## テスト

- 全既存テスト（Unit + Large）がパスすること
- コンパイルエラーのテストケースで、位置が式レベルになっていることを確認
  - 具体的なテスト追加は Step 4 で計画
