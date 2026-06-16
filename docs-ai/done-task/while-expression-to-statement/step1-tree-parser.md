# Step 1: tree_parser の変更

## 概要

構文解析レベルで `while` を式（Expression）から文（Statement）に移動する。

## 変更内容

### 1-1. `Expression` enum から `While` を削除

**ファイル**: `src/tree_parser/expression/mod.rs`

```rust
// 削除
While(Box<LocatedExpression>, Vec<LocatedStatement>),
```

`Expression` enum から `While` バリアントを完全に削除する。

### 1-2. `Statement` enum に `While` を追加

**ファイル**: `src/tree_parser/statement/mod.rs`

```rust
pub enum Statement {
    VariableDeclaration(String, Box<LocatedExpression>, bool, Option<i64>),
    FunctionDeclaration(String, Vec<String>, Vec<LocatedStatement>),
    Continue,
    Break,
    Return(Option<Box<LocatedExpression>>),
    While(Box<LocatedExpression>, Vec<LocatedStatement>),  // 追加
    Expression(Box<LocatedExpression>),
    Invalid(usize),
}
```

### 1-3. 式パーサから while 解析を削除

**ファイル**: `src/tree_parser/expression/mod.rs`

`parse_to_expression_tree_factor` 内の `Keyword::While` マッチアームを削除:

```rust
// 削除
Some((Token::Keyword(Keyword::While), _)) => {
    self.parse_to_expression_tree_while_impl()
}
```

`parse_to_expression_tree_while_impl` メソッド自体も削除。

### 1-4. 文パーサに while 解析を追加

**ファイル**: `src/tree_parser/statement/mod.rs`

`parse_to_statements` 内に `Keyword::While` のマッチアームを追加:

```rust
Token::Keyword(Keyword::While) => {
    let start_pos = token_info.code_pointer;
    self.iter.next(); // while キーワードを消費


    // ':' を期待
    if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Colon) {
        self.skip_to_semicolon();
        // エラーリカバリ
        continue;
    }

    // 条件式をパース
    let (cond, mut cond_errors) = parse_to_expression(self.iter);
    if !cond_errors.is_empty() {
        self.code_parse_error.append(&mut cond_errors);
    }

    // ブロックをパース
    match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
    let (body, mut body_errors) = parse_to_statements(self.iter);
    if !body_errors.is_empty() {
        self.code_parse_error.append(&mut body_errors);
    }
    match_expect_token_unused!(self, self.iter.next(), Token::BraceR);

    // ';' を消費
    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);

    let end_pos = self.current_pos_or(start_pos);
    results.push(LocatedStatement {
        statement: Statement::While(cond, body),
        location: SourceLocation { start: start_pos, end: end_pos },
    });
}
```

注意: 式パーサの `parse_to_expression_tree_while_impl` の実装と同等のロジックだが、
文パーサの規約に従い、末尾の `;` を消費する。

## 確認ポイント

- `Expression::While` を参照していた全ての箇所がコンパイルエラーになること（後続ステップで対応）
- `Statement::While` が正しくパースされること
- エラーリカバリが適切に動作すること
