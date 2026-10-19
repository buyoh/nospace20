# 01: CLI エラー出力の整理

[#16](../code-review-2026-10/16-cli-error-output.md) の対応。エラーをすべて stderr に出し、握りつぶしている stdin 読み取りエラーを報告し、バージョン表示と無視されるオプションを直す。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 現状（再確認結果）

| # | 指摘 | 現在の位置 | 状況 |
|---|---|---|---|
| 1 | `handle_parse_error` が `println!`（stdout）で出力 | `src/bin/nospace20.rs:81-124` | 現存。`(internal: ...)`（debug ビルド）・`line:` 行・ソース行・`^` 行・`... and N more errors` もすべて stdout |
| 2 | stdin 読み取りエラーを `.ok()` で無視 | `src/bin/nospace20.rs:205-208`（`.ok()` は 207 行） | 現存 |
| 3 | `#[command(version = "0.1.0")]` | `src/bin/nospace20.rs:51`、`src/bin/whitespace20.rs:27` | 現存。Cargo.toml は `2.0.0` |
| 4 | `--target` / `-o` を `--mode run` で指定しても無視 | `src/bin/nospace20.rs:64-70`、`src/compile_property.rs:114-129` | 現存。`target` は `default_value_t = CliTarget::Ws` のため、明示指定と既定値を区別できない |
| 5 | whitespace20 がパースエラーを `{:?}` で表示 | `src/bin/whitespace20.rs:85` | 現存。実行時エラーも `RuntimeError` を手書きの `match` で文字列化している（`:147-171`）。`WsRuntimeError` には `Display` 実装がある（`src/base/error/ws_error.rs:29`） |

利用者への影響の確認:

- リポジトリ内のテスト（`tests/`、build.rs 生成テスト）・`tools/` は CLI バイナリを起動していない（`Command::new` は `tests/common/mod.rs` の wsc 起動のみ）。stdout のエラー文字列を検査しているものはない
- 外部の [nospace20-webui](https://github.com/buyoh/nospace20-webui) はサーバ実行で `nospace20` を起動する（`src/app/Services/NospaceExecutionService.ts`）。stdout / stderr を別々に UI へ送り、stderr は `tryParseNospaceErrors` で JSON としての解釈を試み、失敗すればそのまま表示する。したがってエラーを stderr に移すと、UI 上で stdout 扱いだったエラーが stderr 扱い（エラー表示）になるだけで、壊れない
- webui は run モードで `--std` / `--debug` / `--ignore-debug` / `--std-ext` だけを渡し、`--target` は compile モードでのみ渡す。run モードで `--target` / `-o` を拒否しても影響しない

## 設計・方針

### 0. 入出力の処理をテストできる関数に切り出す

1・2 はバグ修正なので、AGENTS.md に従いテストを追加する。バイナリの `main` に入出力が直接書かれているとテストできないため、出力先・入力元を引数で受け取る関数にして、ライブラリ側に置く。

新規モジュール `src/cli_report.rs`（`#[cfg(feature = "cli")] pub mod cli_report;`。既存の `cli_utils.rs` は CLI 引数型の定義に専念させ、責務を分ける）:

```rust
//! CLI の入出力ヘルパ（ソースの読み込み、解析エラーの整形出力）

/// ソースコードを読み込む。読み取り失敗・不正 UTF-8 はエラーとして返す
pub fn read_source(reader: &mut impl std::io::Read) -> std::io::Result<String>;

/// 構文・意味解析エラーを、利用者向けの形式（最大 3 件 + `... and N more errors`）で書き出す
pub fn write_parse_errors(
    errors: &[CodeParseError],
    text: &TextCode,
    out: &mut impl std::io::Write,
) -> std::io::Result<()>;
```

- `write_parse_errors` の出力形式は現行の `handle_parse_error` と同じ（`error: ...`、debug ビルドのみ `  (internal: file:line)`、`line:N column:M`、ソース行、`^` 行）
- `bin/nospace20.rs` の `handle_parse_error` は、`write_parse_errors(&errors, text, &mut std::io::stderr().lock())` を呼んで `process::exit(1)` するだけにする。stderr への書き込み失敗は無視する（`let _ =`。報告先がもうないため）
- ファイル入力は `std::fs::read_to_string` のまま（ファイル読み込みのテストは作らない方針のため。`read_source` は stdin 用）
- `wasm_api/pipeline.rs` の `format_error_details` とキャレット行の生成が重複するが、WASM 側は返り値の構造が異なるため本タスクでは共通化しない

### 1. 構文・意味解析エラーを stderr に出す

0 の `write_parse_errors` の出力先を stderr にする。出力の形式（行の並び・文言）は変えない。

- `nospace20 --mode compile x.ns > out.ws` でエラーが `out.ws` に混ざらなくなる
- stdout が閉じられた場合の panic（`println!` は書き込み失敗で panic する）もこの経路では起きなくなる
- バグ修正なので `handle_parse_error` に原因（エラーを `println!` で stdout に出しており、compile 結果のリダイレクト先に混入していた）と解決策のコメントを書く

範囲外: 正常出力（コンパイル結果の `print!`、プログラム自身の出力、trace 表示）の書き込み失敗（broken pipe）時の扱いは変えない。正常出力は stdout が正しい出力先であり、broken pipe 対策は CLI 全体の出力方式の変更になるため。

### 2. stdin 読み取り失敗をエラーにする

```rust
match read_source(&mut std::io::stdin().lock()) {
    Ok(code) => code,
    Err(err) => {
        eprintln!("error: failed to read stdin: {}", err);
        process::exit(1);
    }
}
```

- 不正 UTF-8 では `std::io::Error`（`InvalidData`、`stream did not contain valid UTF-8`）になるので、`error: failed to read stdin: stream did not contain valid UTF-8` と表示して exit 1
- ファイル入力時の `error: failed to read file '{path}': {err}` と文言の形を揃える
- 「ドメイン上の失敗は戻り値で返す（握りつぶさない）」原則に合わせる修正。バグ修正なのでコード中に原因と解決策のコメントを書く

### 3. バージョン表示を Cargo のバージョンに合わせる

両バイナリの `#[command(version = "0.1.0")]` を `#[command(version)]` に変える。clap は `CARGO_PKG_VERSION` を使うので、Cargo.toml の `version` と常に一致する。

- 現在の Cargo.toml は `2.0.0`（VERSION.md の WIP は `2.0.1`）。Cargo.toml の更新はリリース作業の範囲であり、本タスクでは行わない

### 4. run モードで compile 専用オプションを拒否する

`--target` と `-o/--output` は compile モード専用（help にも `only with --mode=compile` とある）。run モードで指定されたらエラーにする。

`--target` は既定値付きのため、明示指定かどうかを区別できるように `Option` にする。

| 箇所 | 変更前 | 変更後 |
|---|---|---|
| `Args.target`（`bin/nospace20.rs`） | `CliTarget`、`default_value_t = CliTarget::Ws` | `Option<CliTarget>`（既定値なし）。help に `default: ws` を残す（例: `/// Compile target (only with --mode=compile, default: ws)`） |
| `CompileProperty.target`（`compile_property.rs`） | `CompileTarget` | `Option<CompileTarget>`。doc を「指定されたコンパイルターゲット（mode=Compile 時のみ有効。None なら既定の Ws）」に更新 |
| `CompileProperty` の構築（`bin/nospace20.rs:183`） | `target: args.target.into()` | `target: args.target.map(Into::into)` |
| ターゲットの参照 | `property.target` | `CompileProperty::target_or_default(&self) -> CompileTarget`（`self.target.unwrap_or_default()`）を追加し、`validate()` の Json 判定と compile 分岐の `match` で使う |
| `ValidationError` | — | バリアント `CompileOnlyOption(&'static str)` を追加。`Display` は `{} is only valid with --mode=compile`（例: `--target is only valid with --mode=compile`）。他のバリアントは `String` を持つが、ここに入るのはソース中の固定のオプション名だけなので `&'static str` にする |

`validate()` に追加する判定:

```rust
if self.mode == ExecutionMode::Run {
    if self.target.is_some() {
        return Err(ValidationError::CompileOnlyOption("--target"));
    }
    if self.output.is_some() {
        return Err(ValidationError::CompileOnlyOption("--output"));
    }
}
```

- 判定を `CompileProperty` に置き、CLI 引数解析（clap）から切り離してユニットテストできるようにする
- バグ修正（指定が黙って無視されていた）なので、`validate()` の判定箇所に原因と解決策のコメントを書く
- `--std-ext` / `--no-std-ext` も現行の run モード（NospaceVM）では使われないが、拒否しない。[retire-direct-interpreter](../retire-direct-interpreter/01-cli-run-via-whitespace.md) で run モードでも意味を持つようになり、webui も run モードで `--std-ext` を渡しているため
- clap の `conflicts_with` は「別の引数が存在するか」しか見られず、`--mode` の値（既定値 run を含む）による条件は書けないため使わない
- `CompileProperty` / `ValidationError` はライブラリから re-export されているが、利用しているのは `bin/nospace20.rs` だけ。`ValidationError` を網羅 `match` している箇所はクレート内に `Display` 実装しかない。公開型の変更として VERSION.md に記載する

### 5. whitespace20 のエラーを `Display` で表示する

| エラー | 変更前 | 変更後 |
|---|---|---|
| パースエラー | `Parse error: {:?}`（例: `Parse error: NumberOverflow { position: 12 }`） | `Parse error: {}`（例: `Parse error: number at position 12 is out of the 64-bit signed integer range`） |
| 実行時エラー | `RuntimeError` を手書きの `match` で文字列化（`Runtime error: Stack underflow`、`Undefined label: 3` など、バリアントにより接頭辞が異なる） | `eprintln!("Runtime error: {}", e)` の 1 行にまとめる（例: `Runtime error: stack underflow`、`Runtime error: undefined label: 3`） |

- `WsRuntimeError` の `Display` を唯一の文言の定義にし、CLI 側の重複を除く。WASM API など他の利用者と文言が揃う
- `examples/ws_profiler.rs:300` の `format!("VM parse error: {:?}", e)` も `{}` に変える（同じ一本化の目的のため。1 文字の変更）
- 文言の大文字・小文字と接頭辞が変わる。whitespace20 の stderr を検査しているテスト・ツールはない（上記確認済み）
- `position` が「ソースのバイト位置ではなく Space/Tab/LF だけを数えた位置」であることは、メッセージではなく `WsParseError` のドキュメントコメントに書く（[02](02-location-and-doccomments.md)）。メッセージを長くするより、型の定義側で単位を明記するほうが AGENTS.md の「単位を明示」方針に沿うため

## テスト

- `src/compile_property_tests.rs` を新規作成し、`compile_property.rs` から `#[cfg(test)] #[path = "compile_property_tests.rs"] mod tests;` で読み込む（`cli_utils.rs` と同じ形式）
  - run + `target: None` + `output: None` → Ok
  - run + `target: Some(Ws)` → `CompileOnlyOption("--target")`
  - run + `output: Some(..)` → `CompileOnlyOption("--output")`
  - compile + `target: Some(Mnemonic)` + `output: Some(..)` → Ok
  - compile + `target: Some(Json)` → `UnimplementedFeature`（既存挙動の維持）
  - `target_or_default()` が `None` で `Ws` を返す
  - `ValidationError` は `PartialEq` を derive していないため、`matches!(p.validate(), Err(ValidationError::CompileOnlyOption("--target")))` の形で検証する（derive を追加する必要はない）
- `src/cli_report_tests.rs` を新規作成（同じく `#[path]` 形式）。入出力はスタブ（`&[u8]` / `Vec<u8>`）を使い、実ファイル・実 stdin は使わない
  - `read_source`: 正しい UTF-8 → 内容が返る、`b"\xff\xfe"` → `Err`（kind が `InvalidData`）
  - `write_parse_errors`: 位置ありエラー 1 件で `error:` 行・`line:N column:M` 行・ソース行・`^` 行が出る（全角文字を含む行でキャレット位置が表示幅に合う）、位置なしエラーは `error:` 行だけ、エラー 5 件で最初の 3 件と `... and 2 more errors`
  - `(internal: ...)` 行は debug ビルドだけで出るため、検証では `error:` 行・位置の行など特定の行の有無を見る（出力全体の完全一致では比較しない）
- 3・5 と、1・2 のバイナリ側の配線（stderr / stdin を渡す部分）は自動テストを追加せず、次の手動確認を行う

```bash
# 1: エラーが stdout に出ない（out.ws が空、エラーは端末に出る）
printf 'func: __main() { x; };' > tmp/e.ns
cargo run --bin nospace20 -- --mode compile tmp/e.ns > tmp/out.ws; echo $?; wc -c tmp/out.ws
# 2: 不正 UTF-8
printf '\xff\xfe' | cargo run --bin nospace20; echo $?
# 3: バージョン
cargo run --bin nospace20 -- --version; cargo run --bin whitespace20 -- --version
# 4: run モードの --target / -o
printf 'func: __main() {};' > tmp/ok.ns
cargo run --bin nospace20 -- --target mnemonic tmp/ok.ns; echo $?   # error, exit 1
cargo run --bin nospace20 -- -o tmp/x.ws tmp/ok.ns; echo $?          # error, exit 1
cargo run --bin nospace20 -- --mode compile --target mnemonic tmp/ok.ns  # 従来どおり成功
# 5: whitespace20 のパースエラー・実行時エラー（0 除算など）
```

## VERSION.md

WIP に次を追記する。

- `CLI: Compile errors are printed to stderr (were printed to stdout)`
- `CLI: Fail when stdin cannot be read (e.g. invalid UTF-8) instead of treating it as an empty source`
- `CLI: --version shows the package version (was fixed at 0.1.0)`
- `CLI: --target / --output with --mode run is an error (was silently ignored). CompileProperty.target is now Option<CompileTarget>`
- `whitespace20: Print parse and runtime errors with their readable messages`

## チェックリスト

1. [ ] `src/cli_report.rs`（`read_source` / `write_parse_errors`）と `cli_report_tests.rs`
2. [ ] `handle_parse_error` を `write_parse_errors` + stderr に置換（バグ原因・解決策のコメント付き）
3. [ ] stdin 読み取りを `read_source` + エラー終了に置換（バグ原因・解決策のコメント付き）
4. [ ] 両バイナリの `#[command(version)]`
5. [ ] `CompileProperty.target` の `Option` 化、`target_or_default`、`ValidationError::CompileOnlyOption`、`validate()` の判定（バグ原因・解決策のコメント付き）、`compile_property_tests.rs`
6. [ ] whitespace20 のパースエラー `{}` 化、実行時エラーの `match` 削除、ws_profiler の `{:?}` を `{}` に
7. [ ] 手動確認
