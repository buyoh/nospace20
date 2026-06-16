# Step 1: tree_parser への LocatedExpression 導入

## 概要

`Expression` をラップする `LocatedExpression` を導入し、構文木の全式ノードに `SourceLocation` を付与する。

## 型定義の変更

### 新規型: `LocatedExpression`

```rust
// src/tree_parser/expression/mod.rs

/// 位置情報付きの Expression
#[derive(Clone, Debug)]
pub struct LocatedExpression {
    pub expression: Expression,
    pub location: SourceLocation,
}
```

### `Expression` enum の変更

`Box<Expression>` → `Box<LocatedExpression>` に変更:

```rust
// Before
pub enum Expression {
    Operation1(Operator1, Box<Expression>),
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    If(Box<Expression>, Vec<LocatedStatement>, Vec<LocatedStatement>),
    While(Box<Expression>, Vec<LocatedStatement>),
    Block(Vec<LocatedStatement>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
    ArrayAccess(String, Box<Expression>),
    Invalid(usize),
}

// After
pub enum Expression {
    Operation1(Operator1, Box<LocatedExpression>),
    Operation2(Operator2, Box<LocatedExpression>, Box<LocatedExpression>),
    If(Box<LocatedExpression>, Vec<LocatedStatement>, Vec<LocatedStatement>),
    While(Box<LocatedExpression>, Vec<LocatedStatement>),
    Block(Vec<LocatedStatement>),
    Function(String, Vec<Box<LocatedExpression>>),
    Factor(i64),
    Variable(String),
    ArrayAccess(String, Box<LocatedExpression>),
    Invalid(usize),
}
```

変更なしのバリアント: `Block`, `Factor`, `Variable`, `Invalid`

### `Statement` enum の変更

```rust
// Before
pub enum Statement {
    VariableDeclaration(String, Box<Expression>, bool, Option<i64>),
    FunctionDeclaration(String, Vec<String>, Vec<LocatedStatement>),
    Continue,
    Break,
    Return(Option<Box<Expression>>),
    Expression(Box<Expression>),
    Invalid(usize),
}

// After
pub enum Statement {
    VariableDeclaration(String, Box<LocatedExpression>, bool, Option<i64>),
    FunctionDeclaration(String, Vec<String>, Vec<LocatedStatement>),
    Continue,
    Break,
    Return(Option<Box<LocatedExpression>>),
    Expression(Box<LocatedExpression>),
    Invalid(usize),
}
```

変更なしのバリアント: `FunctionDeclaration`, `Continue`, `Break`, `Invalid`

## ExpressionBuilder の変更

### ヘルパーメソッド追加

```rust
impl ExpressionBuilder {
    /// 現在のピーク位置を返す。トークンがなければ 0 を返す。
    fn current_pos(&mut self) -> usize {
        self.iter
            .peek()
            .map(|(_, info)| info.code_pointer)
            .unwrap_or(0)
    }

    /// Expression を LocatedExpression に包む
    fn located(&self, expr: Expression, start: usize, end: usize) -> Box<LocatedExpression> {
        Box::new(LocatedExpression {
            expression: expr,
            location: SourceLocation::new(start, end),
        })
    }
}
```

### 各パースメソッドの変更パターン

全てのパースメソッドの戻り値型を `Box<Expression>` → `Box<LocatedExpression>` に変更する。

#### Factor レベル (parse_to_expression_tree_factor)

```rust
fn parse_to_expression_tree_factor(&mut self) -> Box<LocatedExpression> {
    let start = self.current_pos();
    match self.iter.peek() {
        Some((Token::Number(val), _)) => {
            let val = *val;
            self.iter.next();
            let end = self.current_pos();
            self.located(Expression::Factor(val), start, end)
        }
        Some((Token::Identifier(id), _)) => {
            let id = id.clone();
            self.iter.next();
            if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
                // Function call - end は ')' の直後
                return self.parse_to_expression_tree_function_located(&id, start);
            }
            if let Some((Token::BracketL, _)) = self.iter.peek() {
                // Array access
                self.iter.next();
                let index_expr = self.parse_to_expression_tree_root();
                match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
                let end = self.current_pos();
                return self.located(
                    Expression::ArrayAccess(id, index_expr),
                    start, end,
                );
            }
            let end = self.current_pos();
            self.located(Expression::Variable(id), start, end)
        }
        // ... 他のケースも同様
    }
}
```

#### 二項演算 (parse_to_expression_tree_plus 等)

```rust
fn parse_to_expression_tree_plus(&mut self) -> Box<LocatedExpression> {
    let mut left = self.parse_to_expression_tree_mul();
    loop {
        let op = if let Some(token) = self.iter.peek() {
            match token {
                (Token::Plus, _) => Operator2::Plus,
                (Token::Minus, _) => Operator2::Minus,
                _ => return left,
            }
        } else {
            return left;
        };
        self.iter.next();
        let right = self.parse_to_expression_tree_mul();
        // 左辺の開始位置 〜 右辺の終了位置
        let start = left.location.start;
        let end = right.location.end;
        left = self.located(Expression::Operation2(op, left, right), start, end);
    }
}
```

#### 単項演算 (parse_to_expression_tree_unary)

```rust
fn parse_to_expression_tree_unary(&mut self) -> Box<LocatedExpression> {
    let start = self.current_pos();
    let mut op_stack = vec![];
    // ... (op_stack への push はそのまま)
    let mut left = self.parse_to_expression_tree_factor();
    while let Some(op) = op_stack.pop() {
        let end = left.location.end;
        left = self.located(
            Expression::Operation1(op, left),
            start, end,
        );
    }
    left
}
```

#### If/While/Block 式

```rust
fn parse_to_expression_tree_if_impl(&mut self) -> Box<LocatedExpression> {
    let start = self.current_pos();
    // ... (既存のパースロジック)
    let end = self.current_pos();
    self.located(Expression::If(cond, stats_true, stats_false), start, end)
}
```

### re-export

`src/tree_parser/mod.rs` に `LocatedExpression` の re-export を追加:

```rust
pub use expression::{Expression, LocatedExpression, Operator1, Operator2};
```

## 変更対象ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `src/tree_parser/expression/mod.rs` | `LocatedExpression` 定義、全パースメソッドの戻り値型・内部実装変更 |
| `src/tree_parser/statement/mod.rs` | `Statement` バリアントの型変更、`parse_to_statements` 内対応 |
| `src/tree_parser/mod.rs` | `LocatedExpression` の re-export |

## 留意事項

- `parse_to_expression_tree_function` は関数名トークンの位置を `start` とし、`)` の次のトークンの位置を `end` とする
  - 呼び出し元の `parse_to_expression_tree_factor` で `start` をキャプチャ済みなので、引数として渡す方式が良い
- `Expression::Invalid` もラッパーに包むことで、不正式のソース位置が得られる
- `Expression::Block` はサブ式を持たないので型変更不要、ただし `LocatedExpression` でラップはされる
- テストファイル `src/tree_parser/expression/test.rs` が存在する場合は修正が必要
