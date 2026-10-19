# Step 3: statement/mod.rs の分割

2010 行の `src/tree_parser/statement/mod.rs` を、AST 型定義・文の振り分け・宣言系・定義系・制御構文系・repeat 脱糖に分ける。コードは移すだけで、ロジックは変更しない。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

### 前提

Step 1（cursor 導入・`parse_type_spec` の移動）を先に済ませる。Step 1 で `add_parse_error` / `add_end_error` / `skip_to_semicolon` / `current_pos_or` / `parse_type_spec`（約 110 行）が `statement/mod.rs` から抜けるので、その後の状態を分割する。

### 現在の構成（行番号は 4efa899 時点）

| 行 | 内容 |
|---|---|
| 1-125 | AST 型定義（`AliasParamKind`, `AliasParam`, `AliasArg`, `Statement`, `StructFieldDecl`, `LocatedStatement`） |
| 127-187 | `StatementBuilder` 本体・エラー/位置ヘルパー・`parse_to_statements_block` |
| 189-207 | `let:` / `static:` / `final:` の入口 |
| 208-309 | `constexpr:` 宣言 |
| 310-497 | `alias:` 宣言（識別子エイリアス・ブロックエイリアス・テンプレートインスタンス化） |
| 498-558 | `parse_array_size` |
| 559-634 | `parse_type_spec`（Step 1 で `type_spec.rs` へ移動） |
| 636-985 | 配列の文字列初期化・リスト初期化、変数の初期化式 |
| 986-1162 | `parse_variable_declarations` |
| 1163-1253 | `parse_alias_params`（テンプレート関数の `, alias: kind: name` 列） |
| 1254-1373 | `struct:` 定義（`parse_struct_field` 含む） |
| 1374-1394 | `namespace:` |
| 1395-1463 | `import:` |
| 1464-1602 | `func:` 定義 |
| 1603-1629 | `return:` |
| 1630-1644 | `for:` |
| 1645-1737 | `repeat:` |
| 1738-1859 | `parse_to_statements`（キーワードで振り分け。`while:` / `break:` / `continue:` は振り分けの match 内に直接書かれている） |
| 1861-1866 | 入口 `parse_to_statements(iter)` |
| 1868-2008 | repeat 脱糖ヘルパー（`make_located_expr_at`, `desugar_repeat_form1/2/3`） |

### 分割後の構成

```
src/tree_parser/statement/
├── mod.rs            # StatementBuilder 本体、parse_to_statements（振り分け）、parse_to_statements_block、入口関数（約 200 行）
├── types.rs          # AST 型定義（約 125 行）
├── variable.rs       # let / static / final、配列サイズ・配列初期化・初期化式、parse_variable_declarations（約 610 行）
├── constexpr_alias.rs # constexpr 宣言、alias 宣言（約 290 行）
├── definition.rs     # func（+ parse_alias_params）、struct、namespace、import（約 440 行）
├── control.rs        # return、for、repeat、while、break、continue（約 200 行）
├── repeat_desugar.rs # repeat の For への脱糖（約 145 行）
└── test.rs           # 既存（テストロジックは変更しない。`use` の追加のみ可）
```

分け方の基準:

- **variable.rs**: 変数（実行時の記憶域を持つもの）の宣言。`let` / `static` / `final` はすべて `parse_variable_declarations` を共有し、配列関連のヘルパーもこれからしか呼ばれない
- **constexpr_alias.rs**: コンパイル時に解決される名前の束縛。どちらも `name(...)` / `name { ... }` の複数定義形式を取り、構造が似ている
- **definition.rs**: スコープを作る・名前空間に登録する定義。`parse_alias_params` は `func:` からしか呼ばれないので func と同じファイルに置く
- **control.rs**: 制御構文と脱出文。`while` / `break` / `continue` は振り分けの match から `parse_to_statements_while` / `parse_to_statements_break` / `parse_to_statements_continue` として取り出し、ここに置く（中身はそのまま移す）
- **repeat_desugar.rs**: AST を組み立てる純粋関数で `StatementBuilder` に依存しない。`control.rs` の `parse_to_statements_repeat` だけが使う

いずれのファイルも 700 行未満に収まる。`variable.rs` が最大で、さらに分けるなら配列初期化（`parse_array_string_init` / `parse_array_list_init`、約 250 行）を `variable_array.rs` に出せるが、`parse_variable_declarations` からしか呼ばれないため本タスクでは分けない。

### 実装上の注意

- 各ファイルは `impl<'c, 'b: 'a, 'a> StatementBuilder<'c, 'b, 'a> { ... }` ブロックを持つ（Rust では同じ型の impl を子モジュールに分けて書ける）
- 子モジュールに置いたメソッドは、そのモジュール外（`mod.rs` の振り分けや、別の子モジュール）から呼ぶものだけ `pub(super)` にする。例: `parse_to_statements_variable`, `parse_constexpr_declarations`, `parse_to_statements_func` など振り分けから呼ぶ入口。`parse_array_size` などファイル内だけで使うヘルパーは private のまま
- `mod.rs` に残す `parse_to_statements_block` と `StatementBuilder` のフィールド `cur` は private のままで良い（子モジュールは親モジュールの private 項目を参照できる）
- 子モジュールは `use super::super::type_spec::parse_type_spec;`、`use super::super::expression::parse_expression;` など、使う自由関数を各ファイルで `use` する
- 型定義は `mod.rs` で `mod types; pub use types::*;` として再公開し、`tree_parser/mod.rs` の `pub(crate) use self::statement::{AliasArg, ...}` と `expression/mod.rs` の `use super::{LocatedStatement, Statement}` を変更せずに済ませる
- `test.rs` は `use super::*;` で `mod.rs` のスコープにある `Token` / `Keyword` / `TokenInfo` / `PrettyToken` を使っている。分割で `mod.rs` 自体がこれらを使わなくなり `unused_imports` 警告が出る場合は、`mod.rs` の import は削り、`test.rs` 側に必要な `use` を追加する
- ドキュメントコメント・バグ修正コメントはメソッドと一緒に移し、内容は変えない
- 1 ファイル移すごとに `cargo build` で確認し、全ファイル移した後に `cargo test` を通す。差分は「移動のみ」で確認しやすいよう、Step 1・2 とは別コミットにする

## 検証

- `cargo test`（unit + large）全件成功
- `cargo clippy` で新規警告なし
- `wc -l src/tree_parser/statement/*.rs` でどのファイルも 700 行未満であること

## 関連

- [02-module-splitting](../../done-task/code-design-review/02-module-splitting.md): 他モジュールの分割（完了済み）。本 Step はそこから漏れていた `tree_parser/statement` を扱う
