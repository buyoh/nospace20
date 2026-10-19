# Step 1: ParserCursor の導入と parse_type_spec の共通化

トークン列・エラー列・位置取得を `ParserCursor` にまとめ、`ExpressionBuilder` / `StatementBuilder` が 1 つの cursor を共有するようにする。`parse_type_spec` は cursor を受け取る関数として 1 箇所に置く。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

### 現状の構造

```
StatementBuilder { iter: &mut Peekable, code_parse_error: Vec }   // 再帰のたびに新規作成
  ├─ parse_to_expression_tree_root(self.iter)  → 新しい ExpressionBuilder を作成、(expr, errs) を返す
  │     └─ parse_to_statements(self.iter)       → 新しい StatementBuilder を作成 …
  └─ self.code_parse_error.append(&mut errs)    // 呼び出し側で連結（statement 11 箇所 / expression 3 箇所）
```

- `add_parse_error` / `add_end_error` は両 Builder に同じ実装がある
- 位置取得は `ExpressionBuilder::current_pos()`（EOF で `0`）と `StatementBuilder::current_pos_or(default)` の 2 種類
- `parse_type_spec` は両 Builder に同じ実装がある（74 行 × 2）

### 変更後の構造

```
ParserCursor { iter: &mut Peekable, errors: Vec }                 // parse_to_tree / テスト用の入口で 1 つだけ作る
StatementBuilder  { cur: &mut ParserCursor }
ExpressionBuilder { cur: &mut ParserCursor }
type_spec::parse_type_spec(cur: &mut ParserCursor) -> Result<TypeSpec, usize>
```

子 Builder は同じ cursor を再借用して作るので、エラーは常に `cur.errors` に直接追加される。戻り値のエラー Vec を連結する処理はなくなる。

### ParserCursor（`src/tree_parser/cursor.rs` 新規）

```rust
/// 構文解析中のトークン位置とエラー列を保持するカーソル
///
/// ExpressionBuilder / StatementBuilder が共有し、
/// 再帰的に生成される Builder 間でエラー列（と将来はネスト深度）を引き継ぐ。
pub(super) struct ParserCursor<'b: 'a, 'a> {
    /// 解析対象のトークン列。Builder からは `cur.iter.next()` / `cur.iter.peek()` で直接操作する
    pub(super) iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    /// 解析中に記録したエラー。`Invalid(usize)` はこの Vec のインデックスを指す
    errors: Vec<CodeParseError>,
}

impl<'b: 'a, 'a> ParserCursor<'b, 'a> {
    pub(super) fn new(iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>) -> Self;
    pub(super) fn into_errors(self) -> Vec<CodeParseError>;

    pub(super) fn add_parse_error(&mut self, token_info: &TokenInfo, msg: impl Into<Cow<'static, str>>) -> usize;
    pub(super) fn add_end_error(&mut self, msg: impl Into<Cow<'static, str>>) -> usize;
    /// 記録済みエラー件数（引数解析の停滞判定などで使う）
    pub(super) fn error_count(&self) -> usize;

    /// Step 1.2 用の暫定メソッド（既存の挙動を保つ）。Step 1.4 で current_pos() に置き換えて削除する
    pub(super) fn current_pos_or(&mut self, default: usize) -> usize;
    /// 残りトークン数（peek 済みを含む）
    pub(super) fn remaining_tokens(&self) -> usize;
    /// セミコロンまでスキップし、セミコロン自体も消費する
    pub(super) fn skip_to_semicolon(&mut self);
}
```

- `iter` は `pub(super)` フィールドにする。`self.iter.next()` / `self.iter.peek()` の呼び出しを含む行は expression 約 70 行・statement 約 155 行あり、`self.cur.iter.` への機械的な置換で移行できる。ラッパーメソッド（`cur.next()` 等）は作らない
- `remaining_tokens` / `skip_to_semicolon` はそれぞれ片方の Builder でしか使っていないが、トークン位置の操作なので cursor に置く
- ライフタイム引数は 2 つ（`'b`: トークン列、`'a`: Peekable の可変借用）。既存の Builder と同じで、入口関数のシグネチャ `fn(&mut Peekable<slice::Iter<PrettyToken>>) -> (..., Vec<CodeParseError>)` は変えない（`parse_to_tree` と単体テストのヘルパーがこの形で呼んでいる）
- 07 案 A のネスト深度カウンタはこの構造体に追加する想定（本タスクでは追加しない）

### Builder の変更

```rust
struct ExpressionBuilder<'c, 'b: 'a, 'a> {
    cur: &'c mut ParserCursor<'b, 'a>,
}

/// 共有 cursor 上で式を 1 つ解析する（StatementBuilder から呼ぶ）
pub(super) fn parse_expression(cur: &mut ParserCursor) -> Box<LocatedExpression> {
    ExpressionBuilder { cur }.parse_to_expression_tree_root()
}

/// 入口（parse_to_tree・単体テスト用）。シグネチャは現状維持
pub(super) fn parse_to_expression_tree_root(iter: &mut Peekable<...>) -> (Box<LocatedExpression>, Vec<CodeParseError>) {
    let mut cur = ParserCursor::new(iter);
    let e = parse_expression(&mut cur);
    (e, cur.into_errors())
}
```

`statement` 側も同様に、自由関数 `pub(super) fn parse_statements(cur: &mut ParserCursor) -> Vec<LocatedStatement>` と入口 `parse_to_statements(iter)` を持つ。

- Builder 構造体は private のままにし、モジュールをまたぐ呼び出しは上の自由関数で行う（`impl ExpressionBuilder<'_, '_, '_> { fn parse_with(cur) { Self { cur } } }` のような関連関数にすると、`Self` の匿名ライフタイムが `cur` と結び付かずコンパイルできない。また構造体が private なので他モジュールから型名で呼べない）
- ライフタイム引数の宣言は実装時にコンパイラに合わせて調整してよい

Builder 内部の置換:

| 現在 | 変更後 |
|---|---|
| `self.iter.` | `self.cur.iter.` |
| `self.add_parse_error(` / `self.add_end_error(` | `self.cur.add_parse_error(` / `self.cur.add_end_error(` |
| `match_expect_token!(self, ...)` / `match_expect_token_unused!(self, ...)` | 第 1 引数を `self.cur` にする（`$self.add_parse_error` が cursor のメソッドに解決される。マクロ定義の変更は下記「マクロの修正」の 1 点のみ） |
| `self.code_parse_error.len()` | `self.cur.error_count()` |
| `let (e, mut errs) = parse_to_expression_tree_root(self.iter); self.code_parse_error.append(&mut errs);` | `let e = parse_expression(self.cur);` |
| `let (s, mut errs) = parse_to_statements(self.iter); ... append` | `let s = parse_statements(self.cur);` |
| `self.current_pos()`（expression） | `self.cur.current_pos_or(0)`（Step 1.2 時点では挙動を変えない。Step 1.4 で `self.cur.current_pos()` に置換） |
| `self.current_pos_or(x)`（statement） | `self.cur.current_pos_or(x)`（同上） |

- 移行は Step 1.2（ExpressionBuilder と StatementBuilder の両方）を 1 コミットで行う。片方だけ移行した中間状態では、旧入口が返すエラー Vec を cursor に入れる手段（private な `errors` への追加）が必要になるため、中間状態を作らない
- 子 Builder に渡す `self.cur` は再借用（`&mut *self.cur`）になる。親の `self` を同時に借用する箇所はないので借用の衝突は起きない想定
- `error_index_for_stalled_arg` は「引数解析前のエラー件数より増えていれば最後のエラーを使う」判定で、エラーが共有 Vec に直接積まれるようになっても判定はそのまま成り立つ（引数は同じ Builder の `parse_to_expression_tree_root()` で解析しており、ネストした子のエラーも戻る前に連結済みなので、判定結果は現状と変わらない）

### マクロの修正

`macros.rs` の `match_expect_token!` は `super::describe_expected_token(...)` を呼んでいる。現在の呼び出し元（`expression/mod.rs`, `statement/mod.rs`）では `super` が `tree_parser` なので解決できるが、Step 3 で `statement/variable.rs` などに移すと `super` が `tree_parser::statement` になり、コンパイルできなくなる。

- `super::describe_expected_token` を `$crate::tree_parser::describe_expected_token` に変える（Step 1 で行う。`describe_expected_token` は `pub(super)` なので crate 内から参照できる）
- `tree_parser/mod.rs` に追加する `mod cursor;` / `mod type_spec;` は、`#[macro_use] mod macros;` より後に宣言する（`macro_rules!` はテキスト上の宣言順でスコープが決まるため）

### エラー順序が変わらないことの確認

現状は「子 Builder が溜めたエラーを、子の呼び出しから戻った直後に親の Vec 末尾へ連結」している。子の呼び出し中に親がエラーを積むことはないので、共有 Vec に直接積んだ場合と記録順は同じになる。エラーの件数・メッセージ・順序は変わらない。

変わるのは `Expression::Invalid(i)` / `Statement::Invalid(i)` の `i` で、子のローカル基準から通し番号になる。参照箇所は `semantic_analyzer/expression.rs:656` と `semantic_analyzer/statement.rs:634` の `Invalid(_)` だけなので影響はない。`Expression::Invalid` の NOTE コメント（`expression/mod.rs:86-89`「本来は ExpressionBuilder 単位ではなく、全体で独立したインデックスを利用するべき」）はこの変更で解消するので、「`ParserCursor` のエラー列全体での通し番号」と書き換える。

### parse_type_spec（`src/tree_parser/type_spec.rs` 新規）

```rust
/// 型指定 `&&int[3]` などを解析する。`@` は呼び出し元で消費済み
pub(super) fn parse_type_spec(cur: &mut ParserCursor) -> Result<TypeSpec, usize>
```

- 本体は `expression/mod.rs:702-773` をそのまま移す（`self.` → `cur.`）
- `TypeSpec` の定義は `expression/mod.rs` に残す（`mod.rs` の re-export パスを変えないため）。将来 `type_spec.rs` へ移してもよいが本タスクでは移さない
- 呼び出し元: expression の後置 `@`（1 箇所）、statement の関数引数・戻り値・struct フィールド・変数宣言など。`self.parse_type_spec()` → `parse_type_spec(self.cur)` に置換する
- `parse_struct_field`（`statement/mod.rs:1254-1296`）の配列サイズ解析 `[N]` は `parse_type_spec` の配列部分と似ているが、こちらは `TypeSpec` を作らず `array_size` だけを返すので共通化しない

### 位置取得の統一（Step 1.4）

`ExpressionBuilder::current_pos()` は EOF で `0` を返すため、入力末尾で終わった式の `location` が `start > end` になる。例: トークン列 `[Number(1)@3, Plus@5]` を解析すると、`+` の右辺は EOF で `Invalid`（`start=0, end=0`）になり、ルートの Operation2 は `start=3, end=0` になる。

一方 `StatementBuilder::current_pos_or(default)` は呼び出し側が既定値を渡す方式だが、`parse_to_expression_tree_unary` のように開始位置そのものを `current_pos()` で求める箇所では渡せる既定値がない。そこで「EOF のときの位置」を cursor 自身が持ち、位置取得を引数なしの 1 メソッドに統一する。

```rust
pub(super) struct ParserCursor<'b: 'a, 'a> {
    pub(super) iter: ...,
    errors: Vec<CodeParseError>,
    /// 入力の最後のトークンの code_pointer（トークンがなければ 0）。EOF 時の位置として使う
    eof_pos: usize,
}

impl ParserCursor<'_, '_> {
    pub(super) fn new(iter) -> Self {
        // Peekable<slice::Iter> は Clone。last() は slice::Iter::last() に委譲されるので O(1)
        let eof_pos = iter.clone().last().map(|(_, info)| info.code_pointer).unwrap_or(0);
        ...
    }

    /// 現在のピーク位置の code_pointer。トークンがなければ最後のトークンの位置を返す
    pub(super) fn current_pos(&mut self) -> usize {
        self.iter.peek().map(|(_, info)| info.code_pointer).unwrap_or(self.eof_pos)
    }
}
```

- Step 1.2 の時点では、expression 側を `self.cur.current_pos_or(0)`、statement 側を `self.cur.current_pos_or(x)` として挙動を保つ。Step 1.4 で両方を `self.cur.current_pos()` に置き換え、`current_pos_or` を削除する
- `parse_to_expression_tree_if_impl` の `else` 分岐で自前に書いている `peek().map(..).unwrap_or(else_start)` も `self.cur.current_pos()` に置き換える
- 変更後の EOF 時の値は「最後のトークンの位置」になり、解析中に取得した任意の `start` 以上になる（`start` はどれかのトークンの位置か、それ自体が EOF 位置のため）。したがって `start <= end` が常に成り立つ
- statement 側の EOF 時の `end` は `start_pos` から最後のトークンの位置に変わる。どちらも `start_pos` 以上であり、`SourceLocation.end` はどこからも参照されていないので影響はない
- `iter.clone().last()` は `slice::Iter::last()` に委譲され O(1)。入口は `parse_to_tree` と単体テストからしか呼ばれないので、いずれにせよ 1 解析あたり 1 回
- `TokenInfo` は `code_pointer`（トークンの開始位置）しか持たないため、EOF 時の位置は「最後のトークンの開始位置」になる。例えばファイル末尾の `func: f() {}` の `end` は `}` の開始位置を指し、`}` 自体は含まない（EOF でない場合の `end` は次のトークンの開始位置）。`end` はどこからも参照されないので実害はないが、`current_pos` のドキュメントコメントにこの意味を書いておく
- 追加する単体テスト（`expression/test.rs`）: 上の例 `[Number(1)@3, Plus@5]` を解析し、ルートの `location` が `start=3, end=5` になることを確かめる（変更前は `end=0`）
- バグ修正なので、AGENTS.md に従い `ParserCursor::current_pos` のドキュメントコメントに「EOF で 0 を返していたため end < start になっていた。最後のトークンの位置を返すよう統一した」旨を書く

## 作業手順

1. `cursor.rs` を追加し `mod.rs` に `mod cursor;` を追加
2. 両 Builder を cursor 方式に移行して `append` を削除し、`match_expect_token!` を `$crate::tree_parser::describe_expected_token` に変更（1 コミット。`cargo test` が通ることを確認）
3. `type_spec.rs` を追加し両 Builder の `parse_type_spec` を削除（`cargo test`）
4. 位置取得の統一とテスト追加（3 までとは別コミットにする）

## 検証

- `cargo test`（unit + large）全件成功。tree_parser の単体テスト（expression 44 件・statement 29 件）は変更不要のはず（入口関数のシグネチャを変えないため）
- `resources/tests/fails/syntax/` の期待エラーメッセージが変わらないこと（large テストで確認される）
