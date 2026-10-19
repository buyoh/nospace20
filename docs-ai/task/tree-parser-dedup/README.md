# tree_parser の重複解消・statement モジュール分割

[コードレビュー 2026-10 #17](../code-review-2026-10/17-tree-parser-duplication.md) の対応。
2 つの Builder に重複している共通処理を `ParserCursor` に集約し、2010 行の `statement/mod.rs` を責務ごとに分割する。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 背景・動機

[17-tree-parser-duplication.md](../code-review-2026-10/17-tree-parser-duplication.md) の指摘内容を、現在のソース（コミット 4efa899 時点）で再確認した結果:

| # | 指摘 | 現在の位置 | 状況 |
|---|---|---|---|
| 1 | `parse_type_spec` が 2 箇所に完全重複 | `statement/mod.rs:559-634`、`expression/mod.rs:702-773` | 本体は同一（差分なし） |
| 2 | 引数リストのループが 2 箇所に重複 | `expression/mod.rs:173-227`（関数呼び出し）、`269-324`（構造体リテラル） | 01 の修正（停滞検出）も 2 箇所に同じものが入っている |
| 3 | `add_parse_error` / `add_end_error` / 位置取得が両 Builder に重複 | `expression/mod.rs:111-133`、`statement/mod.rs:144-178` | `current_pos()` は EOF で `0`、`current_pos_or(default)` は `default` を返し挙動が異なる |
| 4 | `statement/mod.rs` が 2010 行 | 同左 | 完了済みの [02-module-splitting](../../done-task/code-design-review/02-module-splitting.md) の対象外だった |
| 5 | 構造体リテラル引数で `args.push((*e).clone())` が Box の中身を深くコピー | `expression/mod.rs:315`（同種が `statement/mod.rs:911` の `parse_variable_init` にもある） | 未修正 |

追加で確認した事項:

- **エラーが Builder ごとのローカル Vec に溜まる**: `ExpressionBuilder` から `parse_to_statements(self.iter)` を呼ぶたび（およびその逆）に新しい Builder とエラー Vec が作られ、戻り値の Vec を `append` で親に連結している。そのため `Expression::Invalid(usize)` / `Statement::Invalid(usize)` のインデックスは子 Builder のローカル Vec 基準であり、最終的なエラー列のインデックスと一致しない。現状このインデックスは参照されていない（`semantic_analyzer` は `Invalid(_)` として無視）ため実害はない
- [07-recursion-depth-limit.md](../../done-task/code-review-2026-10/07-recursion-depth-limit.md) 案 A は「両 Builder に `depth` を持たせる」としているが、Builder は再帰のたびに作り直されるため Builder のフィールドでは深さを引き継げない。共有の状態置き場が必要で、本タスクの `ParserCursor` がそれに当たる
- `SourceLocation.end` は `tree_parser` の外でも単体テストでも参照されていない（`start` は `base/constexpr_eval.rs` 等でエラー位置に使われている）。EOF で位置を取るのは (1) 正常に終わった文の `end`（例: ファイル末尾の `func: f() {}`）と、(2) 式・文の途中で入力が尽きた場合の `start`/`end` の 2 通り。(2) は必ず構文エラーが記録され `parse_to_tree` が `Err` を返すので、その AST は後段に渡らない。したがって EOF 時の位置を変えても後段の動作・エラー位置には影響しない
- [docs-ai/architecture/modules.md](../../architecture/modules.md) の tree_parser の節は `expression.rs`, `statement.rs` と古い構成のまま

## 設計・方針

- 公開 API（`tree_parser::parse_to_tree` と re-export している型）は変更しない。変更は `src/tree_parser/` 内に閉じる
- AST の形は変えない。変わる出力は (a) `Invalid(usize)` のインデックスが最終エラー列の通し番号になる、(b) 入力末尾（EOF）での `location` の位置が、expression では `0`、statement では文の開始位置だったものが、最後のトークンの位置になる、の 2 点だけで、どちらも現在参照箇所がない
- エラーメッセージ・エラーの件数と順序は変えない（`resources/tests/fails/` の large テストがそのまま通ること）
- ステップごとにコミットを分ける。特に出力が変わる Step 1.4（位置取得の統一）は、移動・置換だけの Step 1.1-1.3・Step 3 とは別コミットにする

| ドキュメント | 内容 |
|---|---|
| [01-parser-cursor.md](01-parser-cursor.md) | `ParserCursor` の導入、`parse_type_spec` の共通化、位置取得の統一 |
| [02-call-args-unify.md](02-call-args-unify.md) | 関数呼び出し・構造体リテラルの引数リストを共通化し、Box の深いコピーを除去 |
| [03-statement-module-split.md](03-statement-module-split.md) | `statement/mod.rs` を宣言系・定義系・制御構文系などに分割 |

### 採用しなかった案

- **Builder 同士を 1 つに統合する**: 統合すると `ExpressionBuilder` と `StatementBuilder` の責務が混ざり、ファイルの巨大化がさらに進むので採らない
- **トレイトのデフォルトメソッドで共通化する**（`trait ParserContext { fn cursor(&mut self) -> ...; fn add_parse_error(...) {...} }`）: マクロの呼び出し側を変えずに済むが、Builder の作り直しでエラー Vec と深さが分断される問題は解決しない。状態を 1 つのオブジェクトで共有する案 A を採る

## タスクステップ・チェックリスト

1. [ ] ParserCursor の導入（[01](01-parser-cursor.md)）
   1. [ ] `tree_parser/cursor.rs` に `ParserCursor` を作成し、エラー記録・位置取得・`skip_to_semicolon` を移す
   2. [ ] 両 Builder が `&mut ParserCursor` を持つ形にし、子 Builder にも同じ cursor を渡す（エラー Vec の `append` を廃止。両 Builder を 1 コミットで移行）。`match_expect_token!` の `super::describe_expected_token` を `$crate::tree_parser::describe_expected_token` に変更
   3. [ ] `parse_type_spec` を `tree_parser/type_spec.rs` に移し、両 Builder の重複を削除
   4. [ ] 位置取得を `ParserCursor::current_pos()` に一本化し、EOF 時は最後のトークンの位置を返すよう変更（ExpressionBuilder の EOF 時 `0` を修正、単体テスト追加）。[cli-errors-and-doccomments](../cli-errors-and-doccomments/02-location-and-doccomments.md) が先に入っていれば、`SourceLocation.end` の doc にある EOF 時の注記を消す
2. [ ] 引数リストの共通化（[02](02-call-args-unify.md)）
   1. [ ] `ExpressionBuilder::parse_call_args` を抽出し、関数呼び出し・構造体リテラルから利用
   2. [ ] 構造体リテラル引数の `(*e).clone()` と、`parse_variable_init` の `(*val).clone()` を除去
3. [ ] statement/mod.rs の分割（[03](03-statement-module-split.md)）
   1. [ ] 型定義を `statement/types.rs` に移動
   2. [ ] 各文の解析メソッドを責務ごとのファイルへ移動
4. [ ] 仕上げ
   1. [ ] `cargo test` 全件成功・`cargo clippy` で新規警告なしを確認
   2. [ ] [docs-ai/architecture/modules.md](../../architecture/modules.md) の tree_parser の節を新しい構成に更新
   3. [ ] VERSION.md の WIP に Step 1.4 の変更を 1 行追記（例: `Parser: Source locations of a construct cut off by end of input now point at the last token (was 0 for expressions)`）
   4. [ ] [17-tree-parser-duplication.md](../code-review-2026-10/17-tree-parser-duplication.md) と code-review-2026-10/README.md を完了状態に更新（#17 の行番号は古いので、本ドキュメントの表の行番号に合わせて直す）

## 関連

- [07-recursion-depth-limit.md](../../done-task/code-review-2026-10/07-recursion-depth-limit.md) 案 A: ネスト深度カウンタは `ParserCursor` に置く（本タスクの Step 1 完了後に着手するとよい）。本タスクでは深度カウンタ自体は実装しない
  - （2026-10-19 追記）07-A は [runtime-robustness-2026-10](../../done-task/runtime-robustness-2026-10/03-nesting-depth-and-cli-vm.md) で実装済み。深さは `parse_to_expression_tree_at` / `parse_to_statements_at` の引数で builder 間に渡し、到達した最大深さ `peak` を戻り値で返している。`ParserCursor` 抽出時はこの `depth` / `peak` を移す（[runtime-robustness-followups.md](../runtime-robustness-followups.md) の 2）
- [01-parser-infinite-loop.md](../../done-task/code-review-2026-10/01-parser-infinite-loop.md): 引数ループの停滞検出。Step 2 で 1 箇所に集約する
