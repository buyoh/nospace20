# Step 2: 関数呼び出し・構造体リテラルの引数リスト共通化

`ExpressionBuilder` の関数呼び出しと構造体リテラルにある、ほぼ同じ引数リストのループを 1 つのメソッドにまとめる。あわせて構造体リテラル引数の不要な深いコピーをやめる。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

### 現状の差分

`expression/mod.rs:173-227`（`parse_to_expression_tree_function_located`）と `269-324`（factor の `Keyword::Struct` 分岐）の比較:

| 項目 | 関数呼び出し | 構造体リテラル |
|---|---|---|
| `(` の消費 | ループ前に `match_expect_token!` | 同じ |
| 状態機械 `L / Eval / Comma` | あり | 同じ |
| `)` / `,` / その他 / EOF の各分岐とエラーメッセージ | `unexpected comma` / `missing comma` / `unexpected end of input` | 同じ |
| 停滞検出（01 の修正） | あり | 同じ（コメントも同じ） |
| 引数の格納 | `Vec<Box<LocatedExpression>>` に `push(e)` | `Vec<LocatedExpression>` に `push((*e).clone())` |
| 成功時の戻り値 | `Expression::Function(name, args)` | `Expression::StructLiteral(name, args)` |

違いは引数の格納型と、最後に作る `Expression` のバリアントだけ。

### 抽出するメソッド

```rust
/// `(` 直後から `)` までの引数リストを解析する。`)` も消費する
///
/// 戻り値:
/// - `Ok(args)`: `)` まで解析できた。カンマの過不足は弱いエラーとして記録済みで、解析は続行している
/// - `Err(err_index)`: EOF、または引数の式解析が入力を前進させなかった（呼び出し元は Invalid を返すこと）
fn parse_call_args(&mut self) -> Result<Vec<Box<LocatedExpression>>, usize>
```

- `(` の消費は呼び出し元に残す。`(` が無いときのエラーは呼び出し元ごとに `Invalid` の位置を作っているため
- 本体は関数呼び出し側のループ（173-227）をそのまま移し、`return self.located(...)` の箇所を `return Ok(args)` / `return Err(e)` に置き換える。停滞検出のバグ修正コメント（01 の原因と解決策）はこのメソッドに 1 つだけ残す
- 呼び出し元:

```rust
// 関数呼び出し
match self.parse_call_args() {
    Ok(args) => {
        let end = self.cur.current_pos();
        self.located(Expression::Function(name.clone(), args), start, end)
    }
    Err(e) => {
        let end = self.cur.current_pos();
        self.located(Expression::Invalid(e), start, end)
    }
}

// 構造体リテラル
match self.parse_call_args() {
    Ok(args) => {
        // Box の中身を move で取り出す（従来の (*e).clone() による深いコピーをやめる）
        let args = args.into_iter().map(|e| *e).collect();
        let end = self.cur.current_pos();
        return self.located(Expression::StructLiteral(name, args), start, end);
    }
    Err(e) => { /* 同上の Invalid */ }
}
```

- `end` は現状どちらも `)` 消費後（または停滞・EOF 検出時点）の `current_pos()` であり、`parse_call_args` から戻った直後に取っても同じ値になる
- `Expression::StructLiteral` の要素型を `Vec<Box<LocatedExpression>>` に揃えることはしない（`semantic_analyzer` 側の変更が必要になり、本タスクの範囲 `tree_parser` 内に収まらないため）
- 位置取得は Step 1.4 適用後の `current_pos()` で書いている。Step 1.4 より先に実施する場合は `current_pos_or(0)` とする

### 対象外

- `statement/mod.rs` の型付き構造体初期化 `let: p @ P(1, 2);`（`parse_variable_init`、`statement/mod.rs:903-940`）の値リストは共通化しない。`,` / `)` 以外のトークンで即エラーにして抜けるので停滞は起きず、エラーメッセージも `expected ',' or ')' in struct initializer` と異なるため。ただし同じ深いコピー `values.push((*val).clone())`（911 行）があるので、これは `values.push(*val)` に変える
- `statement/mod.rs` の関数定義の仮引数リスト（`parse_to_statements_func` の `L / Var / Comma` ループ）は、要素が式でなく `識別子 [@ 型]` であり、エラーメッセージ（`expected ','` / `trailing ','`）も異なるため共通化しない

## テスト

- 既存の単体テスト（`expression/test.rs:939` 以降の `assert_stalled_arg_list` を使う停滞系テスト。関数呼び出し・構造体リテラルの両方が既にある（`test_struct_literal_arg_invalid_semicolon_terminates` 等））がそのまま通ることを確認する。テストの追加は不要
- 純粋なリファクタリングなので、バグ修正コメント・VERSION.md の追記は不要
