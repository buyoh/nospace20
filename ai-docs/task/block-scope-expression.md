# ブロックスコープ式の実装

**日付**: 2026-02-10  
**ステータス**: 📋 設計完了

## 概要

`if` や `while` で提供されるスコープ機能のみを提供する、独立したブロックスコープ式 `{ ... }` を追加する。

## 言語仕様

### 構文

```
{ 文... }
```

ブロック `{ }` を単体で式として使用できる。

### セマンティクス

- ブロックは新しいスコープを作成する（`if`/`while` ブロックと同様）
- ブロック内で `let:` で宣言した変数はブロック終了時に破棄される
- ブロックは**式**であるため、文として使う場合は末尾に `;` が必要
- ブロックの値は、最後に評価された式の値
- ブロックが空のとき、値は `0`
- `break`、`continue`、`return` は通常どおり機能する

### 使用例

```nospace
func: main() {
  let: x;
  {
    let: a;
  };
}
```

```nospace
func: main() {
  let: x = {
    let: a;
    a = 3;
    a;
  };
  __clog(x); # x = 3 #
}
```

```nospace
func: main() {
  let: y = {};  # y = 0 (空ブロック) #
}
```

## 設計

### 影響範囲

変更が必要なモジュール:

1. **tree_parser** - 構文解析にブロック式を追加
2. **semantic_analyzer** - ブロック式の意味解析
3. **interpreter** - ブロック式の実行
4. **compiler_ws** - ブロック式の Whitespace コード生成
5. **spec.md** - 言語仕様更新
6. **grammar.bnf** - BNF 更新

### 1. tree_parser (構文解析)

#### Expression 列挙体の変更

`src/tree_parser/expression/mod.rs` の `Expression` に新しいバリアントを追加:

```rust
pub enum Expression {
    // ... 既存のバリアント ...
    Block(Vec<LocatedStatement>),  // 追加
}
```

#### パース処理

`parse_to_expression_tree_factor` に `Token::BraceL` のケースを追加:

```rust
Some((Token::BraceL, _)) => {
    return self.parse_to_expression_tree_block_impl();
}
```

新しいメソッド `parse_to_expression_tree_block_impl`:

```rust
fn parse_to_expression_tree_block_impl(&mut self) -> Box<Expression> {
    self.iter.next(); // '{' を消費
    let (stat, mut stat_err) = parse_to_statements(self.iter);
    if !stat_err.is_empty() {
        self.code_parse_error.append(&mut stat_err);
    }
    match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
    Box::new(Expression::Block(stat))
}
```

#### パース動作の説明

statement パーサの `parse_to_statements` 内で `Token::BraceL` に遭遇した場合:
1. キーワード (`let`, `func`, `return` 等) にマッチしない
2. `BraceR` にもマッチしない
3. フォールスルーで式パーサに渡される
4. 式パーサの `factor` レベルで `BraceL` を検出し、ブロック式としてパースする
5. statement パーサに戻り、`;` を期待する

`parse_to_statements` は `BraceR` を見つけた時点で停止するため、内部のブロック終了と外部のブロック終了が正しく区別される。

### 2. semantic_analyzer (意味解析)

#### ExecExpression 列挙体の変更

```rust
pub(crate) enum ExecExpression {
    // ... 既存のバリアント ...
    Block(Block),  // 追加
}
```

#### 変換処理

`convert_to_exec_expression_with_resolver` に `Expression::Block` のケースを追加:

```rust
Expression::Block(statements) => {
    let (s, es) = analyze_internal_with_parent(
        statements,
        ScopeType::Block,
        Vec::new(),
        Some(parent_resolver),
    )?;
    Ok(Box::new(ExecExpression::Block(Block {
        scope: s.build(false, Vec::new()),
        statements: es,
    })))
}
```

既存の `If`/`While` の処理と同じパターン。`ScopeType::Block` を指定するので、変数のスコーピングは `if`/`while` ブロックと同一。

### 3. interpreter (実行)

#### 式評価

`interpret_expression` に `ExecExpression::Block` のケースを追加:

```rust
ExecExpression::Block(block) => self.interpret_block(block),
```

#### interpret_block メソッド

```rust
fn interpret_block(&mut self, block: &Block) -> ExpressionFlow {
    self.enter_block(&block.scope);
    let (flow, value) = self.interpret_statements_with_value(&block.statements);
    let result = match flow {
        Flow::Proceed => ExpressionFlow::Value(value),
        other => ExpressionFlow::Jump(other),
    };
    self.leave_block();
    result
}
```

これは `interpret_if` の then/else ブロック実行と同一のロジック。

### 4. compiler_ws (Whitespace コード生成)

#### 式のコード生成

`generate_expression` に `ExecExpression::Block` のケースを追加:

```rust
ExecExpression::Block(block) => {
    super::statement::generate_block(ctx, block)
}
```

既存の `generate_block` を呼び出す。現在 `generate_block` はブロック末尾で 0 をプッシュするが、これは `if`/`while` と同じ動作で一貫している。

> **NOTE**: `if`/`while` 式の戻り値機能はインタプリタでは実装済みだが、compiler_ws では未完全。ブロック式も同様に、compiler_ws では常に 0 を返す動作で初期実装する。

### 5. spec.md 更新

セクション 7 「スコープ」の近くに、ブロックスコープ式のセクションを追加する。
または、セクション 6 「制御構文」にブロック式として追加する。

### 6. grammar.bnf 更新

`expr_val` にブロック式を追加:

```bnf
expr_val ::=
    | integer
    | char
    | ident "(" (expr ("," expr)*)? ")"       # 関数呼び出し
    | ident
    | "(" expr ")"
    | block                                     # ブロック式（追加）
```

## 実装手順

### Step 1: tree_parser の変更
- `Expression::Block` バリアントの追加
- `parse_to_expression_tree_block_impl` メソッドの追加
- `parse_to_expression_tree_factor` に `BraceL` ケースの追加
- ユニットテストの追加

### Step 2: semantic_analyzer の変更
- `ExecExpression::Block` バリアントの追加
- `convert_to_exec_expression_with_resolver` の変更
- ユニットテストの追加

### Step 3: interpreter の変更
- `interpret_block` メソッドの追加
- `interpret_expression` に `Block` ケースの追加
- ユニットテストの追加

### Step 4: compiler_ws の変更
- `generate_expression` に `Block` ケースの追加

### Step 5: テストケースの追加
- `resources/tests/passes/` にブロックスコープ式のテストを追加
  - 基本的なスコープ: 変数の宣言と破棄
  - ブロック式の値: 最後の式の値を返す
  - 空のブロック: 0 を返す
  - ネストしたブロック
  - 親スコープ変数へのアクセス

### Step 6: ドキュメント更新
- `spec.md` にブロックスコープ式の説明を追加
- `docs/grammar.bnf` を更新

## 考慮事項

### 空ブロックの扱い

`{}` は空のブロック式として `0` を返す。statement パーサが `BraceR` を見た時点で空の文リストを返し、expression パーサが `BraceR` を消費する。

### if/while との整合性

ブロック式の戻り値は、`if`/`while` と同じメカニズム（`interpret_statements_with_value`）で計算される。唯一の違いは条件式・ループが無い点。

### Clone derive について

`Expression::Block` は `Vec<LocatedStatement>` を保持するが、`Expression` には `#[derive(Clone)]` が既にあるため問題ない。
