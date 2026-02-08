# BNF文法・spec.md 更新設計

## grammar.bnf の変更

### 演算子優先順位コメントの更新

```bnf
# 優先順位（低い順）:
#   1. 代入 (=)                    - 右結合
#   2. 論理OR (||)                 - 左結合、短絡評価
#   3. 論理AND (&&)                - 左結合、短絡評価
#   4. 比較 (==, !=, <, <=, >, >=) - 左結合
#   5. 加減算 (+, -)               - 左結合
#   6. 乗除剰余 (*, /, %)          - 左結合
#   7. 単項 (-, !, &, *)           - 右結合            ← 変更
#   8. インデックス ([])           - 未実装
```

### expr_unary の更新

変更前:

```bnf
expr_unary ::=
    | ("-" | "!") expr_unary
    | expr_postfix
```

変更後:

```bnf
expr_unary ::=
    | ("-" | "!" | "&" | "*") expr_unary
    | expr_postfix
```

### expr_postfix / expr_val のコメント更新

変更前:

```bnf
expr_postfix ::= expr_val
# expr_postfix ::= expr_val ("[" expr "]")?   # 未実装: 配列
# expr_postfix ::= "*" expr_postfix           # 未実装: 間接参照

expr_val ::=
    | integer
    | char
    | ident "(" (expr ("," expr)*)? ")"
    | ident
    | "(" expr ")"
#   | "&" ident                               # 未実装: 参照
```

変更後:

```bnf
expr_postfix ::= expr_val
# expr_postfix ::= expr_val ("[" expr "]")?   # 未実装: 配列

expr_val ::=
    | integer
    | char
    | ident "(" (expr ("," expr)*)? ")"
    | ident
    | "(" expr ")"
```

`&` と `*` は expr_unary に移動したため、expr_postfix / expr_val のコメントを削除。

### 未実装機能リストの更新

```bnf
## 未実装機能
#
# - 複合代入演算子 (+=, -=, *=, /=, %=)
# - 配列 (let: arr[4];)
# - 配列アクセス (arr[i])
# - 参照 (&x)              ← 削除
# - 間接参照 (*p)           ← 削除
# - グローバル変数の初期値指定
# - 16進数リテラル (0x...)
# - 変数の初期値 (let: x(5);)
# - final / const 修飾子
```

## spec.md の変更

### セクション 2.7 の更新

変更前:

```markdown
### 2.7 参照・間接参照演算子 (未実装)
```

変更後:

```markdown
### 2.7 参照・間接参照演算子
```

### セクション 2.8 演算子優先順位の更新

変更前:

```markdown
1. 単項演算子 (`-`, `!`)
```

変更後:

```markdown
1. 単項演算子 (`-`, `!`, `&`, `*`)
```

### 代入の左辺ルールの記述追加（新規）

セクション 2.7 に追記:

```markdown
代入の左辺に `*p` を使用することで、参照先に値を代入できる。

\`\`\`
let: x; let: p;
p = &x;
*p = 42;   # x に 42 が代入される #
\`\`\`
```
