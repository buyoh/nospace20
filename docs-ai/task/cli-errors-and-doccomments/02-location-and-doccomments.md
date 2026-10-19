# 02: 位置情報の単位明記とドキュメントコメントの追加

[#18](../code-review-2026-10/18-location-doc-and-missing-doccomments.md) の対応（EOF 時の end 位置を除く）。挙動は変えず、ドキュメントコメントの修正・追加だけを行う。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 現状（再確認結果）

| # | 指摘 | 現在の位置 | 状況 |
|---|---|---|---|
| 1 | `SourceLocation` の doc が「バイト単位」 | `src/base/location.rs:8-12` | 現存。実際は `parse_to_tokens` が `text.chars().enumerate()` の添字を空白除去の**前に**付けるため、元のソース文字列での文字（`char`）インデックス（`src/token_parser/mod.rs:617-623`） |
| 1 | `TokenInfo.code_pointer` に doc がない | `src/token_parser/mod.rs:75-78` | 現存。`CodeParseError.code_pointer` の構造体 doc には「文字インデックス」とある（`src/base/error/parse_error.rs:10`）が、フィールド自体に doc はない |
| 2 | EOF 時 start > end | `src/tree_parser/expression/mod.rs` の `current_pos()` | **本タスクの範囲外**。[tree-parser-dedup](../tree-parser-dedup/README.md) Step 1.4 で修正する |
| 3 | 構造体・enum の概要 doc 欠落 | 下記「追加するドキュメントコメント」 | 現存 |
| 4 | 誤ったコメント「空白文字も含む」 | `src/token_parser/mod.rs:345` | 現存 |
| 追加 | `WsParseError` の `position` の単位が不明 | `src/base/error/ws_error.rs:51-71` | `whitespace::parser::parse` は Space/Tab/LF 以外の文字を除いた `Vec<WsChar>` の添字を `position` にしている（`src/whitespace/parser.rs:17-25`）。ソースのバイト位置・文字位置とは一致しない（[01](01-cli-error-output.md) の 5 から移した項目） |

`SourceLocation.end` の実際の値: `start` は構文要素の先頭トークンの位置。`end` は構文要素を読み終えた時点で**次にピークしたトークンの位置**（`current_pos()` / `current_pos_or(start)`）で、構文要素の直後のトークンの先頭を指す。間の空白・改行・コメントもこの範囲に入る。入力末尾では [tree-parser-dedup](../tree-parser-dedup/README.md) 適用前後で値が変わるため、doc には EOF 時の具体値を書かない。

## 設計・方針

### 位置の単位を明記する

| 対象 | 変更後の doc（要旨） |
|---|---|
| `SourceLocation`（構造体） | 「ソースコード上の範囲。位置はすべて**元のソース文字列における文字（`char`）インデックス**であり、バイトオフセットではない。`&src[start..end]` のようなバイトでのスライスには使えない（マルチバイト文字で panic する）。行・列への変換は `TextCode::char_index_to_line` を使う」 |
| `SourceLocation.start` | 「構文要素の先頭トークンの文字インデックス」 |
| `SourceLocation.end` | 「構文要素の直後のトークンの文字インデックス（含まない終端の目安。間の空白・コメントを含みうる）。`start` と同じ値の場合もある」。tree-parser-dedup Step 1.4 より前に実装する場合は「入力末尾で切れた式では現状 `start` より小さい値（0）になりうる（構文エラーとして報告され、後段には渡らない）」と注記し、Step 1.4 の実装時にこの注記を消す（tree-parser-dedup 側のチェックリストにも追記する） |
| `TokenInfo.code_pointer` | 「トークンの先頭の文字インデックス（元のソース文字列での `char` 単位。空白除去前の位置。バイトオフセットではない）」 |
| `CodeParseError.code_pointer` | 「エラー位置の文字インデックス（`TokenInfo.code_pointer` と同じ単位）。入力末尾などで位置がない場合は `None`」 |
| `WsParseError`（enum） | 「`position` / `first_position` / `second_position` は、ソースから Space / Tab / LF 以外の文字を除いた列での添字（命令文字の通し番号）。ソースのバイト位置・文字位置ではない」 |

- `TextCode::char_index_to_line` の既存の行コメント `// (line, column)` を doc（`/// 文字インデックスを 0 始まりの (行, 列) に変換する。列も文字単位`）にする

### 追加するドキュメントコメント

挙動から読み取れる概要を 1〜2 行で書く。タプルや意味が型から分からないフィールドには、AGENTS.md の方針に従いフィールド doc も付ける。

| 型 | 位置 | 追加する doc（要旨） |
|---|---|---|
| `Keyword` | `token_parser/mod.rs:11` | 予約語（`func`、`let`、`if` など）を表すトークンの種類 |
| `Token` | `token_parser/mod.rs:34` | トークナイザが出力する字句の種類。値を持つもの（識別子・整数・文字列）は値も保持する |
| `TokenInfo` | `token_parser/mod.rs:76` | トークンに付随する位置情報（上記 `code_pointer` のフィールド doc と合わせて） |
| `PrettyToken` | `token_parser/mod.rs:80` | トークンとその位置情報の組。tree_parser の入力単位 |
| `Statement` | `tree_parser/statement/mod.rs:42` | 文の AST ノード。位置は `LocatedStatement` が持つ |
| `Expression` | `tree_parser/expression/mod.rs:66` | 式の AST ノード。位置は `LocatedExpression` が持つ |
| `Operator1` | `tree_parser/expression/mod.rs:41` | 単項演算子 |
| `Operator2` | `tree_parser/expression/mod.rs:18` | 二項演算子 |
| `TextCode` | `logger/mod.rs:1` | エラー表示用にソースを行単位に分割したもの。文字インデックスから行・列を求める。フィールド `line_indices` に「各行の先頭の文字インデックス」を付ける。コメントアウトされた `source` フィールドの行は削除する |
| `FunctionIndex` | `semantic_analyzer/scope.rs:13` | 関数の解決結果。行末コメント `// (global_index, arg_count, return_type)` を構造体 doc に移し、各要素の意味（`.0`: 関数のグローバルインデックス、`.1`: 引数の数、`.2`: 戻り値の型）を書く。名前付きフィールドへの変更は行わない（参照箇所が `info.0` / `info.1` / `info.2` の 6 箇所に限られ、doc で十分なため） |
| `ImportDecl` | `semantic_analyzer/mod.rs:301` | 名前空間の解析中に集めた `import` 宣言 1 件。`current_ns`（宣言がある名前空間。区切りは `$`、ルートは空文字列）、`target_ns_name`（宣言に書かれた名前空間名。未解決）、`location`（`import` 文の開始文字インデックス。エラー位置に使う）にフィールド doc を付ける。`is_weak` / `is_export` は名前どおりなので省略してよい |
| `ImportBundle` | `semantic_analyzer/mod.rs:309` | import 解決の結果。`import_table` は「名前空間 → import された名前 → 完全修飾名」（`ScopeResolver.import_table` と同じ形）、`export_aliases` は「export された完全修飾名 → import 元の完全修飾名」 |
| `VariableIndex` | `semantic_analyzer/scope.rs:17` | 変数の解決結果（`.0`: 変数のインデックス）。#18 の一覧にはないが同じファイルの同種の型なので一緒に追加する |
| `Identifier` | `semantic_analyzer/scope.rs:20` | スコープに登録された識別子の種類（関数・変数など）。同上 |

- `semantic_analyzer` の型は private だが、AGENTS.md は可視性で区別していないので対象とする
- 対象外:
  - `src/interpreter/` 内の型（`Environment`、`Flow`、`ExpressionFlow` など）。[retire-direct-interpreter](../retire-direct-interpreter/README.md) で削除または `semantic_test/` へ移動・作り直しとなるため、そちらで扱う
  - `Statement` / `Expression` のタプルバリアント（`VariableDeclaration(name, _, _, _, _, _)` など）の要素の意味。要素ごとの doc を付けるにはバリアントを名前付きフィールドに変える必要があり、参照箇所が多い。[tree-parser-dedup](../tree-parser-dedup/README.md) Step 3 で型定義を `statement/types.rs` に移した後に別途検討する（本タスクでは enum の概要 doc のみ）
- doc の文言は実装時に該当コードを読んで確定する。上表は要旨であり、実際の意味と食い違う場合はコードを正とする

### 誤ったコメントの修正

`token_parser/mod.rs:345` の「通常の文字（空白文字も含む - nospace では空白は無視されるが、文字列内では保持）」は誤り。`parse_to_tokens` が字句解析の前に空白文字をすべて除くため、文字列リテラル内にも空白は残らない（docs/spec.md の字句規則と一致）。次のように直す。

```rust
// 通常の文字。空白文字は字句解析の前に除去済みのため、文字列リテラル内にも現れない
// （空白を出力するにはエスケープシーケンスを使う）
```

エスケープシーケンスで空白を表せることは、実装時に `parse_escape_sequence` と spec の記述で確認してからコメントに書く。

`token_parser/mod.rs:38` の `StringLiteral(Vec<i64>)` の行末コメント「各文字のASCII値のベクタ」も誤り。実際に入るのは各文字の Unicode スカラー値（`*c as i64`）と、エスケープシーケンスで指定された値。行末コメントをバリアントの doc にして次のように直す。

```rust
/// 文字列リテラル。各要素は文字の Unicode スカラー値、またはエスケープシーケンスで指定された値（ヌル終端は含まない）
StringLiteral(Vec<i64>),
```

エスケープで指定できる値の範囲（`\x` の桁数など）は、実装時に `parse_escape_sequence` で確認して必要なら書き足す。

## テスト

挙動の変更がないため、テストの追加はない。`cargo doc --no-deps` で doc の書式エラー（壊れたリンクなど）が増えないことを確認する。

## チェックリスト

1. [ ] `SourceLocation`・`TokenInfo.code_pointer`・`CodeParseError.code_pointer`・`WsParseError`・`TextCode::char_index_to_line` の単位を明記
2. [ ] 「追加するドキュメントコメント」の表の型に doc を追加
3. [ ] `token_parser/mod.rs:345`・`:38` のコメント修正
4. [ ] `cargo doc --no-deps` で新しい警告がないことを確認
