# Keyword トークンにコロンを内包する設計

## 概要

token_parser において、Keyword の直後にコロンが来ることを利用し、コロンまで確認した上で Keyword トークンとして認識する（コロンがなければ Identifier として扱う）設計。

## 動機

nospace 言語では空白が無視されるため、`letx` のような文字列は `let` + `x` ではなく `Identifier("letx")` として正しくパースされる。しかし、`let` 単体が出現した場合は Keyword として認識される。Keyword の直後にコロンが必ず来るようにすれば、コロンの有無で Keyword/Identifier を判別できる：

- 予約語を減らし、`let` 等をユーザー変数名として使用可能にする
- token_parser レベルでの曖昧性を排除し、後段パーサーの簡略化

## 前提調査

### 各キーワードのコロン有無（現状）

| キーワード | コロン | 構文例 |
|---|---|---|
| `let` | 常にあり | `let: x;` |
| `static` | 常にあり | `static: x;` |
| `func` | 常にあり | `func: name() {}` |
| `if` | 常にあり | `if: expr {}` |
| `else` | 常にあり | `else: {}` |
| `while` | 常にあり | `while: expr {}` |
| `for` | 常にあり | `for: {} {} {} {};` |
| `repeat` | 常にあり | `repeat: body;` |
| **`return`** | **両方** | `return;` / `return: expr;` |
| **`break`** | **なし** | `break;` |
| **`continue`** | **なし** | `continue;` |

11キーワードのうち3つ（`break`、`continue`、`return` の一部構文）がコロン無し。

## 方針決定

**言語仕様を変更し、全キーワードの直後にコロンを必須化する。**

- `break;` → `break:;`
- `continue;` → `continue:;`
- `return;` → `return:;`（`return;` 形式を廃止）

### 根拠

- 例外が3つのみであり、変更の影響範囲は限定的
- `break:;` / `continue:;` は「コロンの後の引数が空」と解釈でき、`return:;`（void return）と一貫する
- 全キーワードがコロン付きに統一されることで、token_parser でのキーワード判定がシンプルになる

## 実装計画

### Step 1: 言語仕様の更新

対象ファイル:
- `docs/spec.md`
- `docs/grammar.bnf`

変更内容:

```bnf
# Before
return_stmt   ::= "return" ":" expr ";" | "return" ":" ";" | "return" ";"
break_stmt    ::= "break" ";"
continue_stmt ::= "continue" ";"

# After
return_stmt   ::= "return" ":" expr ";" | "return" ":" ";"
break_stmt    ::= "break" ":" ";"
continue_stmt ::= "continue" ":" ";"
```

`docs/spec.md` 内の break/continue の説明・コード例を `break:;` / `continue:;` に更新。
`return;` 形式の記述を削除。

### Step 2: token_parser の変更

対象ファイル: `src/token_parser/mod.rs`

変更概要:
- `determine_keyword_or_identifier` 関数を変更し、キーワード候補の検出後に次の文字がコロン (`:`) であるかを確認する
- コロンが続く場合のみ Keyword トークンを返す（コロンを内包）
- コロンが続かない場合は Identifier として返す
- `Token::Colon` トークンは Keyword 直後には出現しなくなる（Keyword が `:` を内包するため）

注意点:
- `determine_keyword_or_identifier` は現在 `iter` を受け取っていないため、シグネチャを変更するか、呼び出し側の `parse_identifier` でピーク確認を行う必要がある
- `parse_to_tokens_internal` 内の識別子パース直後にコロンをピークする方式が最もシンプル

### Step 3: tree_parser の変更

対象ファイル:
- `src/tree_parser/statement/mod.rs`
- `src/tree_parser/expression/mod.rs`

変更概要:
- Keyword トークンの直後の `Token::Colon` 消費を全箇所で削除
  - `parse_variable_declarations`: `match_expect_token_unused!(... Token::Colon)` 削除
  - `parse_to_statements_func`: `match_expect_token_unused!(... Token::Colon)` 削除
  - `parse_to_statements_return`: コロン消費ロジックと `return;` 分岐を除去し、`return:;` のみに簡素化
  - while の `Token::Colon` 期待を削除
  - `parse_to_statements_for`: `match_expect_token!(... Token::Colon)` 削除
  - `parse_to_statements_repeat`: `match_expect_token!(... Token::Colon)` 削除
  - `parse_to_expression_tree_if_impl`: `match_expect_token!(... Token::Colon)` 削除
  - else 後の `match_expect_token_unused!(... Token::Colon)` 削除
- `break` / `continue` のパース:
  - 現状: キーワード消費 → セミコロン期待
  - 変更: 同じ動作のまま（Keyword トークンがコロンを内包しているため、コロンの処理追加は不要）

### Step 4: Unit テストの更新

対象ファイル:
- `src/tree_parser/statement/test.rs`
- `src/token_parser/test.rs`（存在する場合）

変更概要:
- `test_parse_break_statement`: トークン列にコロンを追加（不要、Keywordがコロン内包のため変更不要。ただし token_parser テストでは更新が必要）
- `test_parse_continue_statement`: 同上
- `test_parse_void_return_without_colon`: このテストは廃止または `return:;` に変更
- `test_parse_return_statement`: トークン列からコロンを削除（Keyword がコロン内包のため）
- `test_parse_void_return_with_colon`: トークン列からコロンを削除

tree_parser のテストでは、Keyword トークンがすでにコロンを内包した状態で渡されるため、
各テストから `token_colon()` の挿入/削除を調整する。

### Step 5: Large テスト（テストリソース）の更新

対象ファイル（`break;` / `continue;` を含むもの）:
- `resources/tests/passes/c002.ns`
- `resources/tests/passes/control_flow/break_continue_001.ns`
- `resources/tests/passes/control_flow/for_break_001.ns`
- `resources/tests/passes/control_flow/for_continue_001.ns`
- `resources/tests/passes/control_flow/repeat_form2_001.ns`
- `resources/tests/passes/control_flow/repeat_form3_001.ns`
- `resources/tests/passes/control_flow/while_expr_value_001.ns`
- `resources/tests/passes/examples/e1-01-queue.ns`
- `resources/tests/fails/compile/break_outside_func_001.ns`
- `resources/tests/fails/compile/continue_outside_func_001.ns`

変更: `break;` → `break:;`、`continue;` → `continue:;`

`return;` 形式のテストリソースは存在しない（調査済み）。

### Step 6: ドキュメント更新

- `docs/tutorial.md`: break/continue の例があれば更新
- `ai-docs/task/self-compiler/`: nospace-core 仕様にも反映が必要な可能性

## 影響分析

### 後方互換性

- `break;`、`continue;`、`return;` の3構文は**破壊的変更**
- 既存の nospace プログラムは修正が必要（機械的な置換で対応可能）

### 他タスクへの影響

- `ai-docs/task/self-compiler/`: nospace-core の break/continue/return 仕様にも同様の変更が必要
- `ai-docs/task/add-elsif-keyword.md`: elsif キーワード追加時にもコロン付きで一貫させる
