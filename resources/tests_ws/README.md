# Whitespace インタプリタ直接テスト

このディレクトリには、`src/whitespace/` の Whitespace パーサ・インタプリタを直接テストするテストケースが含まれています。

## ファイル形式

### WSA (Whitespace Assembly) 記法

- `S` = Space, `T` = Tab, `N` = LF
- `#` で始まる行はコメント
- 上記以外の文字は無視
- ファイル拡張子: `.wsa`

### check.json ファイル

各 `.wsa` ファイルには、対応する `.check.json` ファイルが必要です。

#### ws_io テスト

```json
{
  "type": "ws_io",
  "stdout": "expected output"
}
```

stdin が必要な場合:

```json
{
  "type": "ws_io",
  "stdin": "input data\n",
  "stdout": "expected output"
}
```

#### ws_runtime_error テスト

```json
{
  "type": "ws_runtime_error",
  "error": "ErrorVariantName"
}
```

エラーの種類:
- `StackUnderflow`
- `DivisionByZero`
- `UndefinedLabel`
- `CallStackUnderflow`
- `ProgramCounterOutOfBounds`

## ディレクトリ構造

- `passes/` : 正常系テスト
  - `stack/` : スタック操作
  - `arith/` : 算術演算
  - `heap/` : ヒープアクセス
  - `flow/` : フロー制御
  - `io/` : I/O 操作
- `fails/` : 異常系テスト
  - `runtime/` : 実行時エラー

## テスト実行

```bash
cargo test --test whitespace_direct_test
```

## テスト追加方法

1. `.wsa` ファイルを適切なディレクトリに作成
2. 対応する `.check.json` を作成
3. `test-manifest.yaml` にテスト定義を追加
4. `cargo test` でテストを実行
