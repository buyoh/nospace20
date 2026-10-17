# `__putc` / OutputChar が 128〜255 を UTF-8 の 2 バイトで出力する

- 重要度: **medium**
- 状態: 未着手

## 進捗

- [ ] 修正・テスト追加

## 問題点

- `src/interpreter/environment.rs:142-145`
- `src/whitespace/interpreter.rs:754`

`(val as u8) as char` を `write!` すると Latin-1 文字として UTF-8 エンコードされる。一方 `__getc` はバイト単位で読むため、入力をそのまま出力するプログラムでマルチバイト文字が壊れる。

### 再現: getc → putc のエコープログラムに `"あ"`（`e3 81 82`）を入力

| 実行系 | 出力 |
|---|---|
| nospace20 / whitespace20 | `c3 a3 c2 81 c2 82` |
| wsc | `e3 81 82` |

ws で `push 200; outchar` のみでも whitespace20 は `c3 88`、wsc は `c8`。

## 違反している原則

- 入出力の対称性、wsc との互換性

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `self.stdout.write_all(&[val as u8])` で生バイトを書く | 上記 2 ファイル。WASM 側で stdout を `String::from_utf8_lossy` 等で扱う箇所の確認が必要 |
| B | 256 以上・負値の扱い（wsc は "too large" エラー）を仕様で決め、必要ならエラーにする | `docs/spec.md`、両 VM、[05](05-runtime-error-panic.md) のエラー伝播 |

推奨: A + B。
