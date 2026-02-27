# Step 1: `elif:` の導入

## 概要

既存の `if` 構文（ブロック必須）に `elif:` キーワードを追加する。
`elif:` は内部的に `else: if:` と同等のネストされた `Expression::If` を生成する。

### 変更前

```
if: cond1 { block1 } else: if: cond2 { block2 } else: { block3 };
```

### 変更後（追加）

```
if: cond1 { block1 } elif: cond2 { block2 } else: { block3 };
```

`else: if:` 記法は引き続き使用可能とする（後方互換性）。

## 変更内容

### 1. token_parser

**ファイル**: `src/token_parser/mod.rs`

#### Keyword enum に `Elif` を追加

```rust
pub enum Keyword {
    Let, Func, If, Else, Elif, While, Return, Break, Continue, Static,
}
```

#### `determine_keyword_or_identifier` に `"elif"` マッピングを追加

```rust
"elif" => Token::Keyword(Keyword::Elif),
```

### 2. tree_parser/expression

**ファイル**: `src/tree_parser/expression/mod.rs`

#### `parse_to_expression_tree_if_impl` の else 分岐に `Elif` 処理を追加

現在の処理:

```
then ブロック解析後:
  peek が Else → else: を消費 → peek が If なら再帰、そうでなければ else ブロック
  それ以外 → else なし
```

変更後:

```
then ブロック解析後:
  peek が Elif → elif: を消費 → 条件式 + ブロック を再帰的に解析（If として組立）
  peek が Else → else: を消費 → peek が If なら再帰、そうでなければ else ブロック
  それ以外 → else なし
```

具体的には、`stats_false` を構築する `match` に `Keyword::Elif` のアームを追加:

```rust
Some((Token::Keyword(Keyword::Elif), token_info)) => {
    let elif_start = token_info.code_pointer;
    // elif: cond { block } [elif: ... | else: ...]
    // → Expression::If(cond, then_stats, else_stats) を生成し、
    //   Statement::Expression として包む
    //   （else: if: と同じ AST を生成）
    let if_expr = self.parse_to_expression_tree_if_elif_impl();
    let end_pos = self
        .iter
        .peek()
        .map(|(_, info)| info.code_pointer)
        .unwrap_or(elif_start);
    vec![LocatedStatement {
        statement: Statement::Expression(if_expr),
        location: SourceLocation::new(elif_start, end_pos),
    }]
}
```

#### 新規関数 `parse_to_expression_tree_if_elif_impl`

`elif:` を消費し、条件式 + ブロック を解析し、後続の `elif:` / `else:` を再帰的に処理する。
`parse_to_expression_tree_if_impl` とほぼ同じロジックだが、`if` キーワードの代わりに `elif` キーワードを消費する。

実装方法として、既存の `parse_to_expression_tree_if_impl` をリファクタリングし、キーワード消費部分を共通化するのが望ましい。

```rust
// elif: の解析
fn parse_to_expression_tree_if_elif_impl(&mut self) -> Box<Expression> {
    let token = self.iter.next(); // elif キーワードを消費
    assert!(matches!(token, Some((Token::Keyword(Keyword::Elif), _))));

    if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
        return Box::new(Expression::Invalid(e));
    }
    // 以降は if と同じ: 条件式解析 → ブロック解析 → else/elif 処理
    self.parse_to_expression_tree_if_body()
}
```

共通部分を `parse_to_expression_tree_if_body` として抽出:

```rust
// if/elif の共通ボディ解析（コロン消費後から呼び出す）
fn parse_to_expression_tree_if_body(&mut self) -> Box<Expression> {
    let cond = self.parse_to_expression_tree_root();
    // { block } 解析
    // else / elif 処理
    // Expression::If(cond, then_stats, else_stats) を返す
}
```

### 3. その他のモジュール

- **semantic_analyzer**: 変更なし（AST が同じ `Expression::If` を生成するため）
- **interpreter**: 変更なし
- **compiler_ws**: 変更なし

### 4. テスト

- `elif:` を使用する新しいテストケースを追加
- 既存の `else: if:` テストが引き続き動作することを確認

### 5. ドキュメント

- `docs/spec.md`: if 文のセクションに `elif:` 記法を追加
- `docs/grammar.bnf`: if_stmt の定義に `elif` を追加

## BNF の変更

```bnf
# 変更前
if_stmt ::=
    | "if" ":" expr block ("else" ":" block)? ";"

# 変更後
if_stmt ::=
    | "if" ":" expr block ("elif" ":" expr block)* ("else" ":" block)? ";"
```

## AST への影響

なし。`elif:` は `else: if:` と同一の `Expression::If` ネスト構造を生成する。

```
# elif: cond2 { block2 } else: { block3 }
# ↓
Expression::If(cond2, block2_stmts, block3_stmts)

# if: cond1 { block1 } elif: cond2 { block2 } else: { block3 }
# ↓
Expression::If(cond1, block1_stmts, [Statement::Expression(
    Expression::If(cond2, block2_stmts, block3_stmts)
)])
```
