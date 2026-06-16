# 制御構文の変更（if / while）

## 概要

`if` / `while` の制御構文をコンマ区切り + 式ベースに変更する。
また、`elif:` キーワードを新規導入する。

### 現行構文

```
if: cond { block } else: if: cond { block } else: { block };
while: cond { block };
```

### 新構文

```
if: cond, eval, elif: cond, eval, else: eval;
while: cond, eval;
```

ブロックスコープ式 `{}` が既に実装されているため、`{}` も引き続き使用可能：

```
if: cond1, {
  block1
}, elif: cond2, {
  block2
}, else: {
  block3
};
```

```
while: cond, {
  block
};
```

## 変更の動機

- `{}` をブロック構文ではなく一般的な式として統一する
- `elif:` を導入し、`else: if:` チェインを簡潔に書けるようにする
- 制御構文のトークン区切りを `,` で統一し、他の構文（`let:`, `func:` 等）と一貫性を持たせる（`identifier: expr1, expr2, ...;`）

## 実装ステップ

### Step 1: `elif:` の導入

既存の `if` 構文（ブロック必須）に `elif:` を追加する。
`elif:` は内部的に `else: if:` と同じ AST を生成する。

詳細: [step1-elif.md](step1-elif.md)

### Step 2: `while` の変更

`while` をコンマ区切り + 式ベースに変更する。
ブロックの代わりに式を受け取るようにする。AST・中間表現も変更。

詳細: [step2-while.md](step2-while.md)

### Step 3: `if` の変更

`if` をコンマ区切り + 式ベースに変更する。
`elif:` / `else:` もコンマ区切りに変更。AST・中間表現も変更。

詳細: [step3-if.md](step3-if.md)

### Step 4: テストの修正

ほぼ全てのテストに影響がある。テストケースの構文を新構文に更新する。
現時点で具体的なテストの調査は不要。

詳細: [step4-tests.md](step4-tests.md)

## 影響範囲

| モジュール | 影響 |
|---|---|
| token_parser | `Keyword::Elif` 追加 |
| tree_parser/expression | `if` / `while` パースロジック大幅変更 |
| tree_parser/statement | 変更なし（if/while は式として処理） |
| semantic_analyzer | `ExecExpression::If` / `While` の型変更 |
| interpreter | `interpret_if` / `interpret_while` のロジック変更 |
| compiler_ws | `generate_if_expression` / `generate_while_expression` の変更 |
| docs/spec.md | 仕様書更新 |
| docs/grammar.bnf | BNF 更新 |
| docs/tutorial.md | チュートリアル更新 |
| resources/tests/ | ほぼ全テストケースの構文更新 |
