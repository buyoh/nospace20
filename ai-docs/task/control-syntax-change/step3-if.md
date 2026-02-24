# Step 3: `if` の変更

## 概要

`if` をコンマ区切り + 式ベースの構文に変更する。
Step 1 で導入した `elif:` もコンマ区切りに変更する。

### 変更前（Step 1 完了後）

```
if: cond { block1 } elif: cond { block2 } else: { block3 };
if: cond { block1 } else: if: cond { block2 } else: { block3 };
```

### 変更後

```
if: cond, eval, elif: cond, eval, else: eval;
if: cond, eval, else: if: cond, eval, else: eval;
```

ブロックスコープ式使用:

```
if: cond1, {
  block1
}, elif: cond2, {
  block2
}, else: {
  block3
};
```

## 設計方針

### then/else を `Box<Expression>` に変更

Step 2 の `while` と同様に、ブロック (`Vec<LocatedStatement>`) を式 (`Box<Expression>`) に変更する。

### 現行の AST

```rust
Expression::If(Box<Expression>, Vec<LocatedStatement>, Vec<LocatedStatement>)
//              condition        then_block           else_block
```

### 新しい AST

```rust
Expression::If(Box<Expression>, Box<Expression>, Option<Box<Expression>>)
//              condition        then_expr       else_expr (optional)
```

**else の Optional 化**: 現行では else がない場合に空の `Vec` を使っていたが、新構文では `Option<Box<Expression>>` で明示的に表現する。else がない場合は `None` となり、式の値は `0`。

### 中間表現の変更

```rust
// 変更前
ExecExpression::If(Box<ExecExpression>, Block, Block)

// 変更後
ExecExpression::If(Box<ExecExpression>, Box<ExecExpression>, Option<Box<ExecExpression>>)
```

## 変更内容

### 1. tree_parser/expression

**ファイル**: `src/tree_parser/expression/mod.rs`

#### Expression enum の変更

```rust
// 変更前
If(Box<Expression>, Vec<LocatedStatement>, Vec<LocatedStatement>),

// 変更後
If(Box<Expression>, Box<Expression>, Option<Box<Expression>>),
```

#### `parse_to_expression_tree_if_impl` の変更

```rust
fn parse_to_expression_tree_if_impl(&mut self) -> Box<Expression> {
    let token = self.iter.next(); // if キーワードを消費
    assert!(matches!(token, Some((Token::Keyword(Keyword::If), _))));

    if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
        return Box::new(Expression::Invalid(e));
    }

    self.parse_to_expression_tree_if_body()
}
```

#### `parse_to_expression_tree_if_body` の変更

```rust
// if/elif の共通ボディ（コロン消費後から）
fn parse_to_expression_tree_if_body(&mut self) -> Box<Expression> {
    // 条件式
    let cond = self.parse_to_expression_tree_root();

    // ',' を期待
    if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Comma) {
        return Box::new(Expression::Invalid(e));
    }

    // then 式
    let then_expr = self.parse_to_expression_tree_root();

    // else / elif の処理
    let else_expr = match self.iter.peek() {
        Some((Token::Comma, _)) => {
            // ',' を消費してから後続を確認
            self.iter.next();
            match self.iter.peek() {
                Some((Token::Keyword(Keyword::Elif), _)) => {
                    // elif: cond, eval ...
                    Some(self.parse_to_expression_tree_if_elif_impl())
                }
                Some((Token::Keyword(Keyword::Else), _)) => {
                    // else: eval
                    self.iter.next(); // else を消費
                    match_expect_token_unused!(self, self.iter.next(), Token::Colon);
                    match self.iter.peek() {
                        Some((Token::Keyword(Keyword::If), _)) => {
                            // else: if: → 再帰
                            Some(self.parse_to_expression_tree_if_impl())
                        }
                        _ => {
                            Some(self.parse_to_expression_tree_root())
                        }
                    }
                }
                _ => {
                    // ',' の後に elif/else がない → エラー
                    // または、ここで戻す必要があるかもしれない
                    // 検討事項参照
                    None // TODO: エラーハンドリング
                }
            }
        }
        _ => None, // else なし
    };

    Box::new(Expression::If(cond, then_expr, else_expr))
}
```

**検討事項: コンマの曖昧性**

`if: cond, eval` の `eval` 部分が `,` を含む式（例: 関数呼び出し `f(a, b)`）の場合、`,` は関数引数区切り。これは `parse_to_expression_tree_root` が `(` `)`内のカンマを正しく処理するため問題ない。

ただし、`if` の外側の `,` との区別が問題になる場面がある。例えば:

```
if: cond1, eval1, elif: cond2, eval2;
```

ここで `eval1` の終端は最初の `,` ではなく、`elif:` の前の `,`。
`parse_to_expression_tree_root` は `,` で停止するので、`eval1` は最初のカンマまで読む。

```
parse_to_expression_tree_root("cond1") → cond1
カンマ消費
parse_to_expression_tree_root("eval1, elif: cond2, eval2") → eval1 まで
```

これは正しく動作する。`parse_to_expression_tree_root` は代入演算子まで解析し、`,` では停止するため。

### 2. semantic_analyzer

**ファイル**: `src/semantic_analyzer/mod.rs`, `src/semantic_analyzer/types.rs`

#### ExecExpression の変更

```rust
// 変更前
If(Box<ExecExpression>, Block, Block),

// 変更後
If(Box<ExecExpression>, Box<ExecExpression>, Option<Box<ExecExpression>>),
```

#### `convert_to_exec_expression_with_resolver` の If 処理

```rust
// 変更前
Expression::If(cond, stat1, stat2) => {
    // stat1, stat2 を analyze_internal_with_parent で変換
    // → ExecExpression::If(cond, Block, Block)
}

// 変更後
Expression::If(cond, then_expr, else_expr) => {
    let exec_cond = convert_to_exec_expression_with_resolver(cond, parent_resolver)?;
    let exec_then = convert_to_exec_expression_with_resolver(then_expr, parent_resolver)?;
    let exec_else = match else_expr {
        Some(e) => Some(convert_to_exec_expression_with_resolver(e, parent_resolver)?),
        None => None,
    };
    Ok(Box::new(ExecExpression::If(exec_cond, exec_then, exec_else)))
}
```

### 3. interpreter

**ファイル**: `src/interpreter/exec.rs`

#### `interpret_if` の変更

```rust
// 変更前
fn interpret_if(&mut self, cond: &ExecExpression, then_block: &Block, else_block: &Block) -> ExpressionFlow {
    let cond_val = try_get_value!(self.interpret_expression(cond));
    if cond_val != 0 {
        self.enter_block(then_block);
        // then ブロック実行
    } else {
        self.enter_block(else_block);
        // else ブロック実行
    }
}

// 変更後
fn interpret_if(
    &mut self,
    cond: &ExecExpression,
    then_expr: &ExecExpression,
    else_expr: Option<&ExecExpression>,
) -> ExpressionFlow {
    let cond_val = try_get_value!(self.interpret_expression(cond));
    if cond_val != 0 {
        self.interpret_expression(then_expr)
    } else {
        match else_expr {
            Some(e) => self.interpret_expression(e),
            None => ExpressionFlow::Value(0),
        }
    }
}
```

### 4. compiler_ws

**ファイル**: `src/compiler_ws/expression.rs`

#### `generate_if_expression` の変更

```rust
// 変更前: generate_block で then/else ブロックを生成
// 変更後: generate_expression で then/else 式を生成

fn generate_if_expression(
    ctx: &mut Context,
    cond: &ExecExpression,
    then_expr: &ExecExpression,
    else_expr: Option<&ExecExpression>,
) {
    let else_label = ctx.new_label();
    let end_label = ctx.new_label();

    generate_expression(ctx, cond);
    ctx.emit(JumpIfZero(else_label));

    generate_expression(ctx, then_expr);
    ctx.emit(Jump(end_label));

    ctx.emit(Label(else_label));
    match else_expr {
        Some(e) => generate_expression(ctx, e),
        None => ctx.emit(Push(0)),  // else がない場合は 0
    }

    ctx.emit(Label(end_label));
}
```

#### ネスト変数カウントの変更

```rust
// 変更前
ExecExpression::If(cond, then_block, else_block) => {
    count_nested_vars_in_expression(cond)
    + count_nested_vars_in_block(then_block)
    + count_nested_vars_in_block(else_block)
}

// 変更後
ExecExpression::If(cond, then_expr, else_expr) => {
    count_nested_vars_in_expression(cond)
    + count_nested_vars_in_expression(then_expr)
    + else_expr.as_ref().map_or(0, |e| count_nested_vars_in_expression(e))
}
```

### 5. テスト

- 新構文のテストケースを追加
- `if: cond, expr;`（ブロックなし）の動作確認
- `if: cond, eval, elif: cond, eval, else: eval;` の連鎖
- `else: if:` も引き続き動作することを確認

### 6. ドキュメント

- `spec.md`: if 文のセクションを全面更新
- `docs/grammar.bnf`: if_stmt の定義を更新

## BNF の変更

```bnf
# 変更前
if_stmt ::=
    | "if" ":" expr block ("elif" ":" expr block)* ("else" ":" block)? ";"

# 変更後
if_stmt ::=
    | "if" ":" expr "," expr ("," "elif" ":" expr "," expr)* ("," "else" ":" expr)? ";"
```

## `else: if:` の後方互換性

`else: if:` は引き続きサポートする。
パーサが `else:` の後に `if:` を検出した場合、再帰的に `parse_to_expression_tree_if_impl` を呼び出す。
この動作は Step 1 から変わらない。

## if の値の仕様

- then / else の値は最後に評価された式の値
- `else` がなく `if` が評価されなかった場合、値は `0`
- `{ block }` を使用した場合、ブロックの最終式の値が返される（既存の `Expression::Block` の仕様）
