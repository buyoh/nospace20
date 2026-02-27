# Keyword トークンにコロンを内包する設計の検討

## 概要

token_parser において、Keyword の直後に必ずコロンが来ることを利用し、コロンまで確認した上で Keyword トークンとして認識する（コロンがなければ Identifier として扱う）設計の検討。

## 動機

nospace 言語では空白が無視されるため、`letx` のような文字列は `let` + `x` ではなく `Identifier("letx")` として正しくパースされる。しかし、`let` 単体が出現した場合は Keyword として認識される。Keyword の直後にコロンが必ず来るのであれば、コロンの有無で Keyword/Identifier を判別することで、以下のメリットが期待できる：

- 予約語を減らし、`let` 等をユーザー変数名として使用可能にする
- token_parser レベルでの曖昧性を排除し、後段パーサーの簡略化

## 調査結果: 「Keyword の直後に必ずコロンが来る」は正しいか？

**結論: 正しくない。** `break`、`continue` はコロン無し、`return` はコロンありと無しの両方がある。

### 各キーワードのコロン有無

| キーワード | コロン | 構文例 | 根拠（コード） |
|---|---|---|---|
| `let` | 常にあり | `let: x;` | `parse_variable_declarations` で `Token::Colon` 消費 |
| `static` | 常にあり | `static: x;` | 同上 |
| `func` | 常にあり | `func: name() {}` | `parse_to_statements_func` で `Token::Colon` 消費 |
| `if` | 常にあり | `if: expr {}` | `parse_to_expression_tree_if_impl` で `Token::Colon` 消費 |
| `else` | 常にあり | `else: {}` | 式パーサーで `Token::Colon` 消費 |
| `while` | 常にあり | `while: expr {}` | `parse_to_statements` 内で `Token::Colon` 期待 |
| `for` | 常にあり | `for: {} {} {} {};` | `parse_to_statements_for` で `Token::Colon` 消費 |
| `repeat` | 常にあり | `repeat: body;` | `parse_to_statements_repeat` で `Token::Colon` 消費 |
| **`return`** | **両方** | `return;` / `return: expr;` | `parse_to_statements_return`: セミコロン先読みでコロン無し分岐 |
| **`break`** | **なし** | `break;` | `parse_to_statements`: キーワード消費後、直接セミコロン期待 |
| **`continue`** | **なし** | `continue;` | 同上 |

### grammar.bnf からの確認

```bnf
return_stmt ::= "return" ":" expr ";" | "return" ":" ";" | "return" ";"
break_stmt  ::= "break" ";"
continue_stmt ::= "continue" ";"
```

## 競合・矛盾の分析

### 1. `break` / `continue` がコロンを持たない

最も大きな問題。これらはコロン無しで `break;`、`continue;` と記述するため、「Keyword の直後にコロンが来る」前提が成り立たない。

**対処案:**

- **A. 言語仕様を変更して `break:;` `continue:;` とする** — 一貫性は得られるが、意味的に空のコロン後置が不自然（`break:` の後ろに何も来ない）。
- **B. `break` / `continue` を Keyword 分類から除外し、別のトークン種別にする** — 例: `Token::Break` `Token::Continue` として独立させる。Keyword カテゴリからは外す。
- **C. Keyword を「コロン付きキーワード」「コロン無しキーワード」に二分する** — `KeywordWithColon` と `KeywordBare` に分割。

### 2. `return` のコロン有無の二面性

`return` は `return;`（void return、コロン無し）と `return: expr;`（値付き return）の両方が有効。

**対処案:**

- **A. `return;` 構文を廃止し、`return:;` に統一する** — `return:;` は既に有効な構文なので、`return;`（コロン無し）を deprecated → 廃止とする。
- **B. `return` を `break`/`continue` と同様に特別扱いする** — token_parser で `return` の後がコロンかどうかで分岐。コロンがあれば `Keyword::Return`（コロン内包）、コロンがなければ `Token::Identifier("return")` ではなく別のトークン種別。
- **C. `return` を二つのトークンに分ける** — `Keyword::Return`（コロンあり `return:`）と `Keyword::ReturnVoid`（コロンなし `return`）。

### 3. Keyword を Identifier として使えるようになる副作用

コロン付きでのみ Keyword と認識する場合、`let = 5;` のようなコードが合法になる。これは：

- **利点:** 予約語が存在しなくなり、言語の制約が減る
- **懸念:** 混乱を招く可能性がある（`let = let: x; x;` のような意味の読み取りが困難なコード）

### 4. `else` の特殊性

`else` は文の先頭ではなく、`if` の後に出現するキーワード。`else:` を単一トークンとして扱うことは技術的に可能だが、式パーサー側で分岐が発生する。現在は `Keyword::Else` → `Token::Colon` の2トークンとして処理している。

## 結論

「Keyword の直後に必ずコロンが来る」は **3つのキーワード（`break`、`continue`、`return`の一部構文）について成り立たない**。

この設計を進めるためには、以下のいずれかの方針決定が必要：

1. **言語仕様の変更**: `break:;`、`continue:;`、`return:;` への統一（`return;` 廃止）
2. **Keyword の二分類化**: コロン付きキーワード群とコロン無しキーワード群に分割
3. **部分適用**: `break`、`continue`、`return` を除く8キーワードのみに適用し、これら3つは別扱い

いずれの方針でも実装は可能だが、仕様変更を伴う案は既存コードとの互換性に影響するため、慎重な検討が必要。

## 未決定事項

- 上記方針のいずれを採用するか
- `break`、`continue` にコロンを付けた場合の直感性
- 既存テストケースへの影響範囲
