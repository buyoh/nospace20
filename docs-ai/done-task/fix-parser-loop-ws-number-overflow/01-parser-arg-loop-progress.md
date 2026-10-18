# 01: 引数リストのループで入力の前進を保証する

関数呼び出し `f(...)`・struct リテラル `struct: S(...)` の引数ループで、引数の式解析がトークンを 1 つも消費しなかった場合にループを打ち切り、無限ループとメモリ無制限消費を防ぐ。

作成日: 2026-10-18
ステータス: 完了

## 背景・動機

[code-review-2026-10/01](../../task/code-review-2026-10/01-parser-infinite-loop.md) 参照。採用案は **B**（引数ループで root 呼び出し前後のトークン位置を比較する）。

原因の要点:

- `parse_to_expression_tree_factor` のフォールバック分岐（`src/tree_parser/expression/mod.rs` の `Some((token, token_info)) => { add_parse_error(...); Invalid }`）は、想定外のトークンを**消費せずに**エラーを記録して `Invalid` を返す
- 引数ループ（関数呼び出し: `parse_to_expression_tree_function_located`、struct リテラル: factor 内の `Keyword::Struct` 分岐）は `)` / `,` / EOF 以外のトークンを見ると `parse_to_expression_tree_root()` を呼ぶ。消費されなかったトークンが再び先頭に来るため、同じ位置で root を呼び続け、エラーと引数が無限に push される

factor が不正トークンを消費しない挙動自体は、上位（文パーサ）が `;` や `}` を見て回復するために必要なので変更しない（レビュー 01 の案 A を採らない理由）。

## 設計・方針

### 前進の判定方法

`ExpressionBuilder::iter` は `Peekable<std::slice::Iter<PrettyToken>>` であり、`ExactSizeIterator` を実装している（`Peekable::len()` は peek 済みの要素も数に含める）。root 呼び出し前後の `self.iter.len()`（残りトークン数）を比較し、等しければ「前進なし」と判定する。

- `code_pointer`（ソース上の位置）での比較は、EOF で 0 になる `current_pos()` の仕様と組み合わさって判定が曖昧になるため使わない
- 判定ロジックは小さなヘルパに切り出す（将来 [17](../../task/code-review-2026-10/17-tree-parser-duplication.md) で引数ループを `parse_comma_separated_args()` に統合する際、そのまま移設できるようにする）

```rust
/// 残りトークン数（peek 済みのトークンを含む）。式解析で入力が前進したかの判定に使う
fn remaining_tokens(&self) -> usize {
    self.iter.len()
}
```

### 前進しなかった場合の扱い

**引数ループを打ち切り、引数リスト全体を `Expression::Invalid` として返す。消費されなかったトークンは消費しない（呼び出し元に残す）。**

| 選択肢 | 挙動 | 判断 |
|---|---|---|
| 打ち切って `Invalid` を返す（採用） | `f(;` の `;` は文パーサが文末として扱い、以降の文を通常どおり解析できる | 回復が上位の既存ロジックに任されるので、エラーの連鎖が少ない |
| 1 トークン消費して継続 | `f(;` は `;` を飲み込み、`)` を探して以降の文まで引数として解析し続ける | 後続の文が丸ごと誤った引数になり、無関係なエラーが大量に出る |

エラー記録:

- 前進しなかったということは、factor のフォールバック分岐で `unexpected token ...` が既に記録されている（EOF はループの `None` 分岐で先に処理されるため、ここには来ない）。重複してエラーを追加しない
- `Expression::Invalid(usize)` に渡すインデックスは、直前に記録されたエラー（`self.code_parse_error.len() - 1`）を使う。万一エラーが記録されていない場合（将来の変更で factor の挙動が変わった場合など）に備え、空なら `add_parse_error(token_info, "expected expression")` で新たに記録したインデックスを使う。panic はしない

### 変更箇所

`src/tree_parser/expression/mod.rs` のみ。

1. `ExpressionBuilder` に `remaining_tokens()` を追加
2. `parse_to_expression_tree_function_located` の引数ループ `Some((_, token_info))` 分岐
3. factor 内 `Keyword::Struct` 分岐の引数ループ `Some((_, token_info))` 分岐

2 と 3 は同じ形に変更する（擬似コード）:

```rust
Some((_, token_info)) => {
    if let State::Eval = state {
        self.add_parse_error(token_info, "missing comma");
    }
    // バグ修正: 引数の式が不正トークン（`;` `}` `]` など）で始まると、factor はエラーを
    // 記録するだけでトークンを消費しない。そのため同じ位置で root を呼び続け、
    // 無限ループとメモリ無制限消費が起きていた。
    // 解決策: root 呼び出しで入力が前進しなかった場合は引数リストを打ち切り、
    // 不正トークンは呼び出し元（文パーサ）の回復処理に任せる。
    let before = self.remaining_tokens();
    let e = self.parse_to_expression_tree_root();
    if self.remaining_tokens() == before {
        let err = match self.code_parse_error.len().checked_sub(1) {
            Some(i) => i,
            None => self.add_parse_error(token_info, "expected expression"),
        };
        let end = self.current_pos();
        return self.located(Expression::Invalid(err), start, end);
    }
    args.push(e);
    state = State::Eval;
}
```

`token_info` は `&'b TokenInfo`（トークン列スライスへの参照）であり `self.iter` の借用とは独立しているため、root 呼び出し後もそのまま使える（既存の `"missing comma"` 分岐でも `&mut self` メソッドと併用している）。clone は不要。

### 文パーサ側で停止すること

前進なしで返された不正トークンは文パーサ（`src/tree_parser/statement/mod.rs`）が受け取る。文パーサは `match_expect_token!(self, self.iter.next(), ...)` で期待トークンを `next()` により消費するため、必ず前進する。実装時は下記の再現ケースすべてで解析が終了することを large テストで確認する。

なお、文パーサ側の初期化リストのループ（`parse_array_list_init` など）は「カンマでなければ break」する構造で、同種の無限ループは起きない（確認済み）。

## タスクステップ・チェックリスト

1. [x] `remaining_tokens()` を追加
2. [x] 関数呼び出し引数ループに前進チェックとバグ修正コメントを追加
3. [x] struct リテラル引数ループに同じ変更を追加
4. [x] ユニットテスト追加（`src/tree_parser/expression/test.rs`）
5. [x] large テスト追加（`resources/tests/fails/syntax/` と `test-manifest.yaml`）
6. [x] `cargo test` で既存の syntax_error テストのメッセージが変化していないことを確認
7. [x] `VERSION.md` の WIP セクションに変更内容を追記（例: `Parser: Fix infinite loop on invalid token in argument list`）

### ステップ 4: ユニットテスト

既存ヘルパ `parse_expr(tokens)` は内部で iterator を作って破棄するため、残りトークンを検査できない。以下のヘルパを追加する。

```rust
/// 式を解析し、解析後に iterator 先頭に残っているトークンも返す
fn parse_expr_with_rest(tokens: &[PrettyToken]) -> (Box<LocatedExpression>, Vec<CodeParseError>, Option<Token>) {
    let mut iter = tokens.iter().peekable();
    let (e, errs) = parse_to_expression_tree_root(&mut iter);
    let rest = iter.peek().map(|(t, _)| t.clone());
    (e, errs, rest)
}
```

`;` `}` `]` `(` `)` `struct` 用のトークン生成ヘルパ（既存の `token_op_plus()` などと同じ形式）も必要に応じて追加する。

ハングすればテストが終わらないため、それ自体が回帰検出になる（ただし失敗時は fail ではなく、メモリを急速に消費してハングする。CI のタイムアウトで検出される）。エラーメッセージとエラー件数の検証はユニットテストで行う（large テストのハーネスは後述のとおり `contains` を検査しない）。

| テスト | 入力トークン | 期待 |
|---|---|---|
| 関数呼び出し + `;` | `f` `(` `;` | 戻り値が `Expression::Invalid`、エラー 1 件（`unexpected token`）、残りトークンが `;` |
| 関数呼び出し + `]` | `f` `(` `]` `)` `;` | 同上。`]` が残っている |
| struct リテラル + `;` | `struct` `S` `(` `;` | 同上 |
| 正常な引数の後の不正トークン | `f` `(` `1` `,` `;` | 戻り値が `Invalid`、`;` が残っている |
| 既存挙動の維持 | `f` `(` `1` `2` `)` | `Function` が返り、`missing comma` エラー 1 件（従来どおり） |

### ステップ 5: large テスト

`resources/tests/fails/syntax/` に追加し、`test-manifest.yaml` に `type: syntax_error` で登録する。check.json は既存例（`missing_comma_001`）と同じ形式で、`"phase": "tree"`、`"contains": ["unexpected token"]`。

| ファイル名 | 内容（レビュー記録の再現ケース） |
|---|---|
| `call_arg_invalid_token_001.ns` | `func: __main() { f(; }` |
| `call_arg_invalid_token_002.ns` | `func: __main() { f(]); }` |
| `call_arg_invalid_token_003.ns` | `func: __main() { f(} ` |
| `call_arg_invalid_token_004.ns` | `func: __main() { f([2,3]); }` |
| `call_arg_invalid_token_005.ns` | `func: __main() { f(: }` |
| `struct_literal_arg_invalid_token_001.ns` | `struct: S(a); func: __main() { let: x(struct: S(;)); }` |

注意: large テストのハーネス（`tests/code_test/error_base.rs` の `test_syntax_error_base`）は check.json の `contains` / `error_count` を読み捨てており、「構文解析がエラーで終了すること」しか検査しない。そのため large テストの役割は「ハングせずに終了すること」の回帰検出であり、メッセージ内容はステップ 4 のユニットテストで担保する（ハーネスへの `contains` 検査の追加は [20-test-harness-and-ci.md](../../task/code-review-2026-10/20-test-harness-and-ci.md) の範囲）。check.json の `contains` は、将来ハーネスが対応したときのために記載しておく。

正確な struct 定義構文は spec.md に合わせて実装時に調整する。各ファイル冒頭に `# ... #` コメントで目的（無限ループの回帰テスト）を記載する。
