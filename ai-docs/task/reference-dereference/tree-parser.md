# tree_parser モジュール変更設計

## 対象ファイル

- `src/tree_parser/expression/mod.rs`

## 現状

### Operator1 enum

```rust
pub enum Operator1 {
    Negative,    // -
    LogicalNot,  // !
}
```

### 単項演算子パース (`parse_to_expression_tree_unary`, L186-L200)

```rust
fn parse_to_expression_tree_unary(...) -> ... {
    // Token::Minus → Operator1::Negative
    // Token::Exclamation → Operator1::LogicalNot
    // ループで連続する単項演算子を右結合で処理
}
```

### 乗算パース (`parse_to_expression_tree_mul`, L204-L218)

`Token::Asterisk` を `Operator2::Multiply` として消費。

### 演算子優先順位チェーン（呼び出し順序）

```
assign → or → and → compare → add → mul → unary → factor
```

`mul` が `unary` を呼び出し、`unary` が `factor` を呼び出す。
**`unary` 段階で `*` が単項として消費されれば、`mul` には到達しない。** これはC言語と同じアプローチで正しく動作する。

## 変更内容

### 1. Operator1 に Ref / Deref を追加

```rust
pub enum Operator1 {
    Negative,    // -
    LogicalNot,  // !
    Ref,         // & (参照取得)
    Deref,       // * (間接参照)
}
```

### 2. `parse_to_expression_tree_unary` の拡張

```rust
fn parse_to_expression_tree_unary(...) -> ... {
    loop {
        match current_token {
            Token::Minus => { op = Operator1::Negative; }
            Token::Exclamation => { op = Operator1::LogicalNot; }
            Token::Ampersand => { op = Operator1::Ref; }
            Token::Asterisk => { op = Operator1::Deref; }
            _ => break;
        }
        advance();
        let inner = self.parse_to_expression_tree_unary(...)?;
        return Ok(Expression::Operation1(op, Box::new(inner)));
    }
    self.parse_to_expression_tree_factor(...)
}
```

### 3. `*` の曖昧性の解消

`Token::Asterisk` は以下の2つの文脈で使われる:

1. **二項乗算**: `a * b` — `parse_to_expression_tree_mul` で処理
2. **単項デリファレンス**: `*p` — `parse_to_expression_tree_unary` で処理

呼び出し順序が `mul → unary` であるため:

- `mul` は左辺の式をパースした後、次のトークンが `*` なら乗算として処理
- `unary` は式の開始位置で `*` を見たらデリファレンスとして処理

この区別は式の開始位置かどうかで自然に決まる。例:

```
a * b    → mul パースで: a (mul) unary(b)
*p       → unary パースで: Deref(factor(p))
a * *p   → mul パースで: a (mul) unary(*p) → a (mul) Deref(factor(p))
**p      → unary パースで: Deref(Deref(factor(p)))
```

### 4. `&` の構文制約

BNF コメントでは `&` の対象を `ident` に限定している:

```bnf
expr_val ::= "&" ident    # 未実装: 参照
```

しかし tree_parser 段階では `& expr_unary` として一般的にパースし、意味解析 (semantic_analyzer) 段階で「対象が変数かどうか」を検証する設計とする。

理由:
- tree_parser は構文解析のみを担当し、意味的な制約は semantic_analyzer が担う
- 将来 `&arr[i]` 等を許容する場合に tree_parser の変更が不要

ただし、BNF のコメントにある `expr_postfix` レベルで `*` を配置する案（後置演算子的扱い）との違いに注意。現設計では `*` を単項前置演算子として扱う。

## `*` を expr_unary に配置する根拠

BNF コメントでは `expr_postfix` に `*` を配置する案がある:

```bnf
# expr_postfix ::= "*" expr_postfix  # 未実装: 間接参照
```

しかし、C言語と同様に `*` を単項前置演算子（`expr_unary` レベル）として扱う方が:

1. `-`, `!` と同じ処理パターンで実装できる
2. `*-p` のような式（負のアドレスをデリファレンス、通常は無意味だが文法上は許容）が自然に扱える
3. `**p` （ダブルポインタ）が自然に右結合で処理される

## テスト

### パーサユニットテスト

```
&x      → Operation1(Ref, Variable("x"))
*p      → Operation1(Deref, Variable("p"))
**p     → Operation1(Deref, Operation1(Deref, Variable("p")))
&(x)    → Operation1(Ref, Variable("x"))  ← 括弧は剥がされるが意味的にOK
a * b   → Operation2(Multiply, Variable("a"), Variable("b"))
a * *p  → Operation2(Multiply, Variable("a"), Operation1(Deref, Variable("p")))
*p = 5  → Operation2(Assign, Operation1(Deref, Variable("p")), Factor(5))
```
