# Step 2: `while` の変更

## 概要

`while` をコンマ区切り + 式ベースの構文に変更する。

### 変更前

```
while: cond { block };
```

### 変更後

```
while: cond, eval;
```

`eval` は任意の式。ブロックスコープ式 `{}` を使えば従来同様のブロックも記述可能:

```
while: cond, {
  block
};
```

## 設計方針: ブロック vs 式

### 現行の AST

```
Expression::While(Box<Expression>, Vec<LocatedStatement>)
```

第2要素はブロック内の文リスト。

### 新しい AST

```
Expression::While(Box<Expression>, Box<Expression>)
```

第2要素を式（`Box<Expression>`）に変更する。
`{ block }` が来た場合は `Expression::Block(stmts)` として自然に表現される。

### 中間表現の変更

```
// 変更前
ExecExpression::While(Box<ExecExpression>, Block)

// 変更後
ExecExpression::While(Box<ExecExpression>, Box<ExecExpression>)
```

`Block` 構造体は `ExecExpression::Block(Block)` に包まれる形になる。

**理由**: `while: cond, expr;` のように `expr` がブロックでない場合、`Block` を生成するのは不自然。式ベースの方が自然な表現になる。

## 変更内容

### 1. tree_parser/expression

**ファイル**: `src/tree_parser/expression/mod.rs`

#### Expression enum の変更

```rust
// 変更前
While(Box<Expression>, Vec<LocatedStatement>),

// 変更後
While(Box<Expression>, Box<Expression>),
```

#### `parse_to_expression_tree_while_impl` の変更

```rust
fn parse_to_expression_tree_while_impl(&mut self) -> Box<Expression> {
    let token = self.iter.next(); // while キーワードを消費
    assert!(matches!(token, Some((Token::Keyword(Keyword::While), _))));

    if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
        return Box::new(Expression::Invalid(e));
    }
    let cond = self.parse_to_expression_tree_root();

    // 変更: '{' の代わりに ',' を期待
    if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Comma) {
        return Box::new(Expression::Invalid(e));
    }

    // 変更: ブロックではなく式を解析
    let body = self.parse_to_expression_tree_root();

    Box::new(Expression::While(cond, body))
}
```

### 2. semantic_analyzer

**ファイル**: `src/semantic_analyzer/mod.rs`, `src/semantic_analyzer/types.rs`

#### ExecExpression の変更

```rust
// 変更前
While(Box<ExecExpression>, Block),

// 変更後
While(Box<ExecExpression>, Box<ExecExpression>),
```

#### `convert_to_exec_expression_with_resolver` の While 処理

```rust
// 変更前
Expression::While(expr, stat) => {
    let (s, es) = analyze_internal_with_parent(stat, ...)?;
    Ok(Box::new(ExecExpression::While(
        convert_to_exec_expression_with_resolver(expr, parent_resolver)?,
        Block { scope: s.build(...), statements: es },
    )))
}

// 変更後
Expression::While(expr, body) => {
    Ok(Box::new(ExecExpression::While(
        convert_to_exec_expression_with_resolver(expr, parent_resolver)?,
        convert_to_exec_expression_with_resolver(body, parent_resolver)?,
    )))
}
```

**注意**: body が `Expression::Block(stmts)` の場合、`ExecExpression::Block(Block{...})` として変換される。Block 内の変数宣言・スコープ管理は `Expression::Block` の変換ロジックが担当する。これは既存の `Expression::Block` 処理と同じ。

### 3. interpreter

**ファイル**: `src/interpreter/exec.rs`

#### `interpret_while` の変更

```rust
// 変更前
fn interpret_while(&mut self, cond: &ExecExpression, block: &Block) -> ExpressionFlow {
    loop {
        let cond_val = try_get_value!(self.interpret_expression(cond));
        if cond_val == 0 { break; }
        self.enter_block(block);
        let flow = self.interpret_statements_with_value(&block.statements);
        // ... break/continue 処理
    }
    ExpressionFlow::Value(0)
}

// 変更後
fn interpret_while(&mut self, cond: &ExecExpression, body: &ExecExpression) -> ExpressionFlow {
    loop {
        let cond_val = try_get_value!(self.interpret_expression(cond));
        if cond_val == 0 { break; }
        let result = self.interpret_expression(body);
        match result {
            ExpressionFlow::Value(_) => { /* 次のイテレーションへ */ }
            ExpressionFlow::Jump(Flow::Continue) => { continue; }
            ExpressionFlow::Jump(Flow::Break) => { break; }
            ExpressionFlow::Jump(flow) => { return ExpressionFlow::Jump(flow); }
        }
    }
    ExpressionFlow::Value(0)
}
```

**重要**: `body` が `ExecExpression::Block(block)` の場合、`interpret_expression` が `interpret_block` を呼び出し、ブロック内のスコープ管理（`enter_block` / 変数の確保・解放）が自動的に行われる。

### 4. compiler_ws

**ファイル**: `src/compiler_ws/expression.rs`

#### `generate_while_expression` の変更

```rust
// 変更前: generate_block(ctx, body) を呼び出し
// 変更後: generate_expression(ctx, body) を呼び出し
```

`body` が `ExecExpression::Block` の場合は `generate_block` が呼ばれ、そうでなければ通常の式生成が行われる。

#### ネスト変数カウントの変更

`count_nested_vars_in_expression` も `While` の第2引数が `Block` から `Box<ExecExpression>` に変わるため更新が必要。

```rust
// 変更前
ExecExpression::While(cond, block) => {
    count_nested_vars_in_expression(cond) + count_nested_vars_in_block(block)
}

// 変更後
ExecExpression::While(cond, body) => {
    count_nested_vars_in_expression(cond) + count_nested_vars_in_expression(body)
}
```

### 5. テスト

- 新構文 `while: cond, eval;` のテストケースを追加
- `while: cond, { block };`（ブロックスコープ式使用）の動作確認

### 6. ドキュメント

- `spec.md`: while 文のセクションを更新
- `docs/grammar.bnf`: while_stmt の定義を更新

## BNF の変更

```bnf
# 変更前
while_stmt ::= "while" ":" expr block ";"

# 変更後
while_stmt ::= "while" ":" expr "," expr ";"
```

`expr` にはブロックスコープ式 `{ stmt* }` も含まれるため、`block` の非終端記号は不要になる。

## スコープの扱い

### `while: cond, { block };` の場合

`{ block }` は `Expression::Block` → `ExecExpression::Block(Block{scope, stmts})` となるため、ブロックスコープ内で宣言された変数はブロック終了時に破棄される。これは現行の動作と同等。

### `while: cond, expr;` の場合（ブロックなし）

`expr` は単なる式。スコープは作成されない。例:

```
let: i(10);
while: i, i = i - 1;
```

この場合、`i = i - 1` は外側のスコープの変数 `i` を直接操作する。
