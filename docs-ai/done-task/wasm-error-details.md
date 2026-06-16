# WASM API エラー詳細情報の追加

## 完了状況

**完了** (2026-03-05)

### 実施内容

1. `Cargo.toml` — `wasm` feature に `dep:unicode-width` を追加
2. `src/wasm_api/types.rs` — `WasmError` に `details: Option<String>` フィールドを追加、TypeScript 型定義に `details?: string;` を追加、`ResultErr::single_error` で `details: None` を明示
3. `src/wasm_api/pipeline.rs` — `format_error_details` ヘルパー関数を追加し、`convert_errors` / `convert_compile_error` で `details` を生成するよう変更、ユニットテスト4件を追加

### テスト結果

- `cargo test`（default/cli feature）: 全て通過
- `cargo test --features wasm wasm_api`: 4件追加テスト全て通過

## 概要

WASM API のエラーレスポンス (`WasmError`) に `details` フィールドを追加し、CLI と同等の詳細なエラーログを返せるようにする。

## 背景・課題

### CLI のエラー出力（現状）

CLI (`src/bin/nospace20.rs` の `handle_parse_error`) は以下の情報を出力する：

```
error: unexpected token: expected Token::Semicolon
  (internal: src/tree_parser/statement/mod.rs:118)
line:7 column:10
  (*next)[0] = tail;
         ^
```

- `error: <メッセージ>`
- デバッグビルド時のみ `(internal: <ファイル>:<行>)` （`CodeParseError.caller` フィールド）
- `line:<行> column:<列>`
- 該当ソースコード行
- キャレット (`^`) でエラー位置を指示

### WASM API のエラー出力（現状）

WASM API は `WasmError` 構造体を返す：

```typescript
interface WasmError {
    message: string;
    line?: number;
    column?: number;
}
```

JS 側ではこれを組み立てて `message:line:column` のような簡素なメッセージしか出せない：

```
unexpected token: expected Token::Semicolon:7:10
```

ソースコード行やキャレット表示がないため、CLI と比較して大幅に情報が少ない。

## 設計

### TypeScript 型定義の変更

`WasmError` に `details` フィールドを追加する：

```typescript
interface WasmError {
    message: string;
    line?: number;
    column?: number;
    details?: string;
}
```

`details` は、CLI が出力するような複数行の詳細エラー文字列を格納する。

### `details` の内容

`details` には以下を含む（存在する情報のみ）：

```
line:7 column:10
  (*next)[0] = tail;
         ^
```

- `line:<行> column:<列>` 行 ― エラー箇所の行・列番号（1-indexed）
- ソースコードの該当行
- キャレット (`^`) によるエラー箇所の視覚的指示

`message` フィールドは変更せず、従来どおりエラーメッセージのみを保持する（後方互換性維持）。

### 変更対象ファイル

#### 1. `src/wasm_api/types.rs` — `WasmError` 構造体

```rust
#[derive(Serialize)]
pub struct WasmError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,  // 追加
}
```

TypeScript 型定義 (`TS_TYPES`) も同様に `details?: string;` を追加。

`ResultErr::single_error` メソッドでは `details: None` とする。

#### 2. `src/wasm_api/pipeline.rs` — エラー変換関数

`convert_errors` と `convert_compile_error` で `details` を生成する。
`TextCode` からソースコード行を取得し、キャレットを生成して `details` フィールドに設定する。

**`convert_errors` の変更：**

```rust
pub(super) fn convert_errors(errors: &[CodeParseError], text: &TextCode) -> ResultErr {
    let wasm_errors: Vec<WasmError> = errors
        .iter()
        .map(|e| {
            let (line, column, details) = if let Some(p) = e.code_pointer {
                let (l, c) = text.char_index_to_line(p);
                let line_1 = l + 1;
                let col_1 = c + 1;
                let details = format_error_details(text, l, c);
                (Some(line_1), Some(col_1), Some(details))
            } else {
                (None, None, None)
            };
            WasmError {
                message: e.message.to_string(),
                line,
                column,
                details,
            }
        })
        .collect();

    ResultErr {
        success: false,
        errors: wasm_errors,
    }
}
```

**`convert_compile_error` の変更：**

同様に `details` フィールドを生成。

**ヘルパー関数 `format_error_details` の追加：**

```rust
/// エラー詳細文字列を生成する（CLI の出力に相当）
///
/// 出力例:
/// ```text
/// line:7 column:10
///   (*next)[0] = tail;
///          ^
/// ```
fn format_error_details(text: &TextCode, line_0: usize, column_0: usize) -> String {
    let line_str = text.line(line_0);
    let line_1 = line_0 + 1;
    let col_1 = column_0 + 1;
    let prefix: String = line_str.chars().take(column_0).collect();
    let width = unicode_width::UnicodeWidthStr::width(prefix.as_str());
    format!(
        "line:{} column:{}\n{}\n{}^",
        line_1,
        col_1,
        line_str,
        " ".repeat(width)
    )
}
```

#### 3. `Cargo.toml` — 依存関係

`unicode-width` クレートが WASM ビルドで利用可能か確認が必要。CLI (`src/bin/nospace20.rs`) では既に `unicode_width::UnicodeWidthStr` を使用しているため、依存関係自体は存在する。ただし、`wasm_api` モジュールから参照可能であることを確認する。

## 後方互換性

- `WasmError` に `details` フィールドを追加するが、`#[serde(skip_serializing_if = "Option::is_none")]` により `None` の場合はシリアライズされない
- 既存の `message`, `line`, `column` フィールドは変更なし
- JS 側で `details` を利用しない既存コードは影響を受けない

## テスト方針

- `pipeline.rs` の `convert_errors` / `convert_compile_error` に対するユニットテストで `details` フィールドが期待通りの文字列を含むことを確認
- WASM ビルドが通ることを確認（`cargo build --target wasm32-unknown-unknown`）
- 既存テスト (`cargo test`) がパスすることを確認

## 作業ステップ

1. `src/wasm_api/types.rs` の `WasmError` 構造体と TypeScript 型定義に `details` フィールドを追加
2. `src/wasm_api/pipeline.rs` に `format_error_details` ヘルパー関数を追加
3. `convert_errors` と `convert_compile_error` で `details` を生成するよう変更
4. `ResultErr::single_error` で `details: None` を設定
5. `cargo test` で既存テストが通ることを確認
6. WASM ビルドの動作確認
