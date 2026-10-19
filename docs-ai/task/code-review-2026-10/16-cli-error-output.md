# CLI: エラー出力先が不統一、stdin 読み取りエラーを握りつぶす、バージョン表示の不一致

- 重要度: **low**
- 状態: 計画 docs-ai/task/cli-errors-and-doccomments/README.md

## 進捗

- [ ] エラー出力を stderr に統一
- [ ] stdin 読み取りエラーの処理
- [ ] バージョン表示修正

## 問題点

- （2026-10-19 追記）`src/bin/whitespace20.rs` はパースエラーを `Parse error: {:?}` で表示するため、`WsParseError::NumberOverflow` などの `Display` メッセージが使われない。`{}` に変更すること。なお `position` はソースのバイト位置ではなく Space/Tab/LF のみを数えた位置である旨もメッセージかドキュメントで示すとよい（[fix-parser-loop-ws-number-overflow](../../done-task/fix-parser-loop-ws-number-overflow/README.md) のレビューで指摘）

- `src/bin/nospace20.rs:81-123`: `handle_parse_error` が `println!`（stdout）を使う。他のエラーは `eprintln!`。
  - `nospace20 --mode compile x.ns > out.ws` でエラー本文が out.ws に書き込まれる
  - stdout がパイプで閉じられると `println!` が panic し終了コード 101
- `src/bin/nospace20.rs:161`: `read_to_string(...).ok()` で stdin 読み取りエラーを無視。不正 UTF-8（`printf '\xff\xfe' | nospace20`）が空ソース扱いになり `function '__main' not found` という誤解を招くメッセージになる（ファイル入力時は正しくエラー）。
- `src/bin/nospace20.rs:51`、`src/bin/whitespace20.rs:27`: `#[command(version = "0.1.0")]` が Cargo.toml の `version = "2.0.0"` と不一致（`--version` で 0.1.0 と表示）。
- `--target` / `-o` を `--mode run` と同時指定しても検証されず黙って無視される。

## 違反している原則

- エラーハンドリングの一貫性、AGENTS.md「ドメイン上の失敗は戻り値で返す」（握りつぶしの禁止）

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `handle_parse_error` を `eprintln!` に統一 | `bin/nospace20.rs`。build.rs 生成の large テストや tools 側が stdout のエラー文字列を検査していないか確認が必要 |
| B | stdin 読み取りを `match` し、失敗時 `error: failed to read stdin: ...` で終了 | `bin/nospace20.rs` |
| C | `#[command(version)]` で Cargo のバージョンを使う | `bin/*.rs` |
| D | run モードで compile 専用オプション指定時に警告またはエラー | `bin/nospace20.rs`、`compile_property.rs` |
