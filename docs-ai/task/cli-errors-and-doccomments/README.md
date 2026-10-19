# CLI エラー出力の整理と位置情報・ドキュメントコメントの修正

コードレビュー 2026-10 の low 指摘 2 件をまとめて対応する。
[#16](../code-review-2026-10/16-cli-error-output.md)（CLI のエラー出力先・stdin エラー・バージョン表示・無視されるオプション）と、
[#18](../code-review-2026-10/18-location-doc-and-missing-doccomments.md)（位置情報の単位の誤記・ドキュメントコメント不足）。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 背景・動機

どちらも挙動上の重大な不具合ではないが、利用者・開発者を誤解させる。

- #16: `nospace20 --mode compile x.ns > out.ws` でコンパイルエラーの本文が `out.ws` に書き込まれる。不正 UTF-8 を stdin から渡すと、空のソースとして扱われ `function '__main' not found` という見当違いのエラーになる。`--version` が `0.1.0`（Cargo.toml は `2.0.0`）と表示される
- #18: `SourceLocation` の doc が「バイト単位」と書いているが、実際は文字インデックス。doc を信じて `&src[start..end]` で切り出すとマルチバイト文字で panic する。AGENTS.md の「構造体の概要は必ずドキュメントコメント」「長さの単位を明示」に反する箇所がある

ソース（コミット 4efa899 + 作業ツリー）で再確認した結果、指摘内容はすべて現存する。ただし #18 の「EOF 時に start > end」は、[tree-parser-dedup](../tree-parser-dedup/README.md) Step 1.4 で `ParserCursor::current_pos()` に一本化して修正する設計がすでにあるため、本タスクでは扱わない（下記「他タスクとの関係」）。

## 設計・方針

- 変更は CLI バイナリ（`src/bin/`）・新規の `src/cli_report.rs`（CLI の入出力をテストできる形に切り出したもの、`cli` feature のみ）・`compile_property.rs` と、ドキュメントコメントだけにする。コンパイラ・VM の挙動は変えない
- #18 は doc の修正・追加のみ。`code_pointer` のリネーム（`char_index` など）は行わない（86 箇所に波及し、[tree-parser-dedup](../tree-parser-dedup/README.md) と大きく衝突するため）

| ドキュメント | 内容 |
|---|---|
| [01-cli-error-output.md](01-cli-error-output.md) | エラーの stderr 統一、stdin 読み取りエラー、バージョン表示、run モードで compile 専用オプションを拒否、whitespace20 のエラー表示を `Display` に統一 |
| [02-location-and-doccomments.md](02-location-and-doccomments.md) | `SourceLocation` / `TokenInfo.code_pointer` の単位明記、欠落ドキュメントコメントの追加、誤ったコメントの修正 |

### 採用しなかった案

| 案 | 不採用の理由 |
|---|---|
| `code_pointer` を `char_index` にリネーム（#18 案 A の前半） | doc で単位を明記すれば誤用は防げる。リネームは token_parser / tree_parser / テスト / `CodeParseError` / CLI / WASM に広く波及し、並行して設計済みの tree-parser-dedup と衝突する |
| run モードで compile 専用オプションを警告のみにする（#16 案 D の「警告」） | 既存の `validate()` は不正な組み合わせをエラーにしている（`--target=json`）。警告は見落とされやすく、方針を揃えてエラーにする。利用者側の webui は run モードで `--target` / `-o` を渡していないことを確認済み |

## 他タスクとの関係

| タスク | 関係 | 対応 |
|---|---|---|
| [tree-parser-dedup](../tree-parser-dedup/README.md) | #18 の「EOF 時 end 位置」は同タスク Step 1.4 で修正される。また同タスクは `tree_parser/expression/mod.rs`・`statement/mod.rs` を大きく移動・分割する | EOF 位置は本タスクの範囲外。`Statement` / `Expression` / `Operator1` / `Operator2` の doc 追加は数行なので、どちらが先でも衝突は小さい（後から着手する側が取り込む） |
| [retire-direct-interpreter](../retire-direct-interpreter/README.md) | 01 で `bin/nospace20.rs` の Run 分岐を書き換える。同タスク 01 も `compile_property.rs` を変更する（`run_compile_options` とそのユニットテストの追加、`ExecutionMode::Run`・`target`・`target_extensions` の doc 修正）。02・03 で `src/interpreter/` は削除・移動される | 本タスクは Run 分岐の中身（`run_with_vm` など）には触れない。`handle_parse_error`・ソース読み込み・`#[command]`・`validate()` だけを変える。whitespace20 の `Display` 統一は本タスク 01 で行う。`compile_property.rs` と `compile_property_tests.rs` は両タスクで共有し、後から着手する側が既存のテストファイルに追記し、`target` などの doc は両方の変更（Option 化・run モードでの扱い）を反映した文言にまとめる。`interpreter/` 内の型の doc 欠落は対象外（[02](02-location-and-doccomments.md)） |
| [ws-vm-run-mode-support](../ws-vm-run-mode-support/README.md) | 01 で `whitespace20` に `--max-memory-cells` と `MemoryLimitExceeded` のエラー表示を追加する。02 で `whitespace20` の clog writer に stderr を設定する | 後から着手する側が取り込む。本タスクで whitespace20 のエラーを `Display` 表示に統一すれば、`MemoryLimitExceeded` も `Display` 実装だけで表示される |
| [runtime-robustness-2026-10](../../done-task/runtime-robustness-2026-10/README.md) | 作業ツリーに `bin/nospace20.rs` の未コミット変更（NospaceVM 移行）がある | その変更のコミット後に着手する |

## タスクステップ・チェックリスト

1. [ ] CLI エラー出力の整理（[01](01-cli-error-output.md)）
   1. [ ] `src/cli_report.rs` に `read_source` / `write_parse_errors` を切り出し、ユニットテストを追加
   2. [ ] `handle_parse_error` の出力を stderr に変更
   3. [ ] stdin 読み取り失敗をエラーにする
   4. [ ] `#[command(version)]` で Cargo のバージョンを使う（nospace20 / whitespace20）
   5. [ ] `CompileProperty::validate` で run モードの `--target` / `-o` を拒否（ユニットテスト追加）
   6. [ ] whitespace20 のパースエラー・実行時エラーを `Display` で表示
2. [ ] 位置情報・ドキュメントコメント（[02](02-location-and-doccomments.md)）
   1. [ ] `SourceLocation` / `TokenInfo.code_pointer` / `WsParseError` の位置の単位を明記
   2. [ ] 欠落している構造体・enum の概要ドキュメントコメントを追加
   3. [ ] 誤ったコメント（`token_parser/mod.rs` の「空白文字も含む」「各文字のASCII値」）を修正
3. [ ] 仕上げ
   1. [ ] `cargo test` 全件成功、`cargo clippy` で新規警告なし
   2. [ ] VERSION.md の WIP に CLI の変更を追記（[01](01-cli-error-output.md) の「VERSION.md」参照。02 は doc のみのため記載しない）
   3. [ ] README.md の CLI 説明に影響がないか確認（エラー出力例・`--version` の記載があれば更新）
   4. [ ] [16-cli-error-output.md](../code-review-2026-10/16-cli-error-output.md)・[18-location-doc-and-missing-doccomments.md](../code-review-2026-10/18-location-doc-and-missing-doccomments.md)・code-review-2026-10/README.md を完了状態に更新（18 の EOF 項目は tree-parser-dedup へ移管と記載）
