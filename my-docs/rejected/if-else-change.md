# 制御構文の変更（if / while）

要約

* 関数呼び出し内でのコンマ曖昧性が発生
* `<else:> {if: 文}` のような設計になってた

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


## レビュー結果

### 「半角スペース・改行を無視」要件との整合性

**問題なし。** 新構文のトークン列はスペース除去後も一意に解釈可能です。

```
if:cond,eval,elif:cond,eval,else:eval;
```

`,`、`elif`（キーワード）、`:` はいずれも構造的に区別でき、曖昧性はありません。

---

### 曖昧な定義・未解決の問題

#### 1. コンマの先読み消費問題（step3-if.md の TODO）

`then_expr` の後の `,` を**消費してから**次のトークンが `elif`/`else` かを判定しています。もし `elif` でも `else` でもなかった場合、消費済みの `,` を戻せません。

```
if: cond, eval, unknown_token;
```

→ `,` が消費された後に `unknown_token` が来ると、パーサの状態が壊れます。
**対策案**: `,` のあとの2トークン目をpeekして `elif`/`else` であることを確認してから `,` を消費するか、エラーリカバリを明確に定義する必要があります。

#### 2. `if`/`while` 式がネスト可能かどうかが未定義

現在のBNF（grammar.bnf）では `if`/`while` は `expr_val` に含まれず、`stmt` レベルのみで出現します。しかしドキュメントでは「式ベースに変更する」とあります。

もし `expr_val` にも追加されるなら、**関数呼び出し内でのコンマ曖昧性が発生します**：

```
foo(if: cond, eval, other_arg)
```

→ パーサは `other_arg` の前の `,` を「if式の elif/else 導入」として消費しようとし、`other_arg` を正しく第2引数として認識できません。

**対策案**: `if`/`while` は引き続きステートメントレベルのみで使用可能であることをドキュメントに明記するか、`expr_val` に追加する場合は括弧等で囲む文法を定義する必要があります。

#### 3. `break`/`continue`/`return` がブロックなしの body で使えるか未定義

```
while: cond, break;
```

`break` はステートメント（`break;`）として定義されていますが、新構文では body が「式」です。`break` を式として扱えるのかどうかが明記されていません。使えないなら `while: cond, { break; };` とブロックが必須になり、ブロックなし形式の実用性が制限されます。

#### 4. `elif` が新たな予約語になることが未記載

step1-elif.md では `Keyword::Elif` を追加しますが、`elif` という名前の変数を使っている既存コードが壊れるリスクについて言及がありません。docs/spec.md の予約語一覧への追加も必要です。

#### 5. `else: if:` の後方互換性テストパターンが step4 に不足

step4-tests.md の変換パターンでは `else: if:` → `elif:` への変換しか示されていませんが、`else: if:` を**そのまま使い続ける**場合の新構文パターンの例がありません：

```
# else: if: を維持する場合
if: cond1, {
  body1
}, else: if: cond2, {
  body2
}, else: {
  body3
};
```

---

### 問題なしと判断した点

| 項目 | 判断 |
|---|---|
| `parse_to_expression_tree_root` が `,` で停止する前提 | BNF上 `,` は式の演算子でないため正しい |
| `{}` ブロックスコープ式との互換 | `Expression::Block` で自然に表現される |
| while の戻り値が常に 0 | 現行仕様と同じ（変更なし） |
| `elif:` が `else: if:` と同一ASTを生成 | semantic_analyzer 以降の変更が不要で合理的 |
| Step 順序の妥当性 | Step 1(elif追加) → Step 2(while) → Step 3(if) の順は依存関係が正しい |