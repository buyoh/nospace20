# Phase 1: 構文解析 (tree_parser) の変更

## 概要

配列宣言 `let: arr[N];` と配列アクセス `arr[i]` の構文解析を実装する。

## 変更ファイル

- `src/tree_parser/statement/mod.rs` — 変数宣言の配列対応
- `src/tree_parser/expression/mod.rs` — `ArrayAccess` バリアント追加 + postfix パース

## 1. Expression enum の拡張

### 変更前

```rust
pub enum Expression {
    Operation1(Operator1, Box<Expression>),
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    If(Box<Expression>, Vec<LocatedStatement>, Vec<LocatedStatement>),
    While(Box<Expression>, Vec<LocatedStatement>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
    Invalid(usize),
}
```

### 変更後

```rust
pub enum Expression {
    Operation1(Operator1, Box<Expression>),
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    If(Box<Expression>, Vec<LocatedStatement>, Vec<LocatedStatement>),
    While(Box<Expression>, Vec<LocatedStatement>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
    ArrayAccess(String, Box<Expression>),   // 追加: arr[expr]
    Invalid(usize),
}
```

`ArrayAccess(name, index_expr)`:
- `name`: 配列変数の識別子名
- `index_expr`: インデックス式

**注意**: `ArrayAccess` は `Variable` と同レベルで変数名を持つ。
任意の式に対する postfix `[...]` ではなく、識別子に対する `[...]` のみサポート。
これは仕様上、配列はスタック変数としてのみ存在し、式の結果が配列になることはないため。

## 2. Statement::VariableDeclaration の拡張

### 変更前

```rust
Statement::VariableDeclaration(String, Box<Expression>, bool)
// (name, init_expr, is_static)
```

### 変更後

```rust
Statement::VariableDeclaration(String, Box<Expression>, bool, Option<i64>)
// (name, init_expr, is_static, array_size)
```

`array_size`:
- `None`: 通常の変数
- `Some(n)`: サイズ n の配列

## 3. 配列宣言のパース (`parse_variable_declarations`)

現在の `parse_variable_declarations` は以下のフロー:

```
let: → Colon → Identifier → ( → init_expr → ) → ;
                           └─→ ; (初期化なし)
```

配列対応後のフロー:

```
let: → Colon → Identifier → [ → Number → ] → ( → init_vals → ) → ;
                           └─→ ( → init_expr → ) → ;        (通常変数の初期化)
                           └─→ ;                             (初期化なし)
```

### 擬似コード

```rust
fn parse_variable_declarations(&mut self, start_pos: usize, is_static: bool) -> Vec<LocatedStatement> {
    // ... (Colon 消費済み)
    loop {
        let id = /* Identifier を取得 */;

        // --- ここから新規追加 ---
        // 配列サイズのチェック
        let array_size = if let Some((Token::BracketL, _)) = self.iter.peek() {
            self.iter.next(); // '[' を消費

            // サイズは定数（Number）のみ
            let size = match self.iter.next() {
                Some((Token::Number(n), _)) => {
                    if *n <= 0 {
                        // エラー: 配列サイズは正の整数
                        return /* error */;
                    }
                    *n
                }
                _ => return /* error: expected array size */,
            };

            // ']' を消費
            match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
            Some(size)
        } else {
            None
        };
        // --- 新規追加ここまで ---

        // 初期化式のチェック
        let init_expr = if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
            if let Some(arr_size) = array_size {
                // 配列の初期化: (val1, val2, val3)
                // 複数の代入文に展開
                parse_array_init(&id, arr_size)
            } else {
                // 通常変数の初期化: (expr)
                parse_scalar_init(&id)
            }
        } else {
            Box::new(Expression::Factor(0))
        };

        results.push(LocatedStatement {
            statement: Statement::VariableDeclaration(id, init_expr, is_static, array_size),
            ...
        });

        // カンマ or セミコロン チェック
    }
}
```

### 配列初期化の展開

`let: arr[3](10, 20, 30);` は以下の文列に展開:

```rust
// Statement::VariableDeclaration("arr", Factor(0), false, Some(3))
// Statement::Expression(arr[0] = 10)
// Statement::Expression(arr[1] = 20)
// Statement::Expression(arr[2] = 30)
```

ただし、tree_parser レベルでの展開は `LocatedStatement` のベクタを返す `parse_variable_declarations` の
既存の仕組みと整合する。`parse_variable_declarations` は既に `Vec<LocatedStatement>` を返す
（複数変数宣言 `let: a, b, c;` のサポートのため）。
配列初期化の代入文もこのベクタに追加する。

## 4. 配列アクセスのパース

`arr[i]` は、`parse_to_expression_tree_factor` で `Identifier` を読んだ後の分岐として実装。

### 変更箇所: `parse_to_expression_tree_factor`

```rust
Some((Token::Identifier(id), _)) => {
    self.iter.next();
    if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
        return self.parse_to_expression_tree_function(id);
    }
    // --- ここから新規追加 ---
    if let Some((Token::BracketL, _)) = self.iter.peek() {
        self.iter.next(); // '[' を消費
        let index_expr = self.parse_to_expression_tree_root();
        match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
        return Box::new(Expression::ArrayAccess(id.clone(), index_expr));
    }
    // --- 新規追加ここまで ---
    return Box::new(Expression::Variable(id.clone()));
}
```

**注意**: `arr[i][j]` のような多次元アクセスは未サポート（spec にもない）。

## 5. テスト項目

### Unit テスト (tree_parser)

- `let: arr[4];` → `VariableDeclaration("arr", ..., false, Some(4))`
- `let: arr[3](10, 20, 30);` → 宣言 + 代入文3つ
- `arr[0]` → `ArrayAccess("arr", Factor(0))`
- `arr[i+1]` → `ArrayAccess("arr", Operation2(Plus, Variable("i"), Factor(1)))`
- `arr[0] = 5;` → `Operation2(Assign, ArrayAccess("arr", Factor(0)), Factor(5))`
- エラーケース: `let: arr[0];` → エラー（サイズ0以下）
- エラーケース: `let: arr[x];` → エラー（変数は不可）

## 6. 考慮事項

### `Clone` derive

`Expression` は `#[derive(Clone)]` がついている（`// TODO: REMOVE` コメント付き）。
`ArrayAccess` も `Clone` を実装する必要がある → Box<Expression> が Clone なので自動的に対応。

### `static` 配列

`static: arr[4];` もパース可能にする。`is_static` フラグは既存の仕組みで対応。

### 複数変数宣言との組み合わせ

`let: a, arr[3], b;` のように、通常変数と配列を混在させることも可能（自然に対応可能）。
