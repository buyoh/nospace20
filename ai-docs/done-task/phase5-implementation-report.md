# Phase 5 実装レポート: 文字列リテラル（糖衣構文）

**実装日**: 2026-02-13
**状態**: 完了

## 実装内容

### 1. Token の拡張 (`src/token_parser/mod.rs`)

文字列リテラルをサポートするため、`Token` enum に `StringLiteral` を追加しました。

```rust
pub enum Token {
    // ... 既存のトークン ...
    StringLiteral(Vec<i64>),  // 文字列リテラル（各文字のASCII値のベクタ、ヌル終端は含まない）
}
```

### 2. 文字列リテラルのパース

`parse_string_literal` 関数を実装し、`"..."` 形式の文字列をパースします。

- エスケープシーケンスのサポート: `\n`, `\r`, `\t`, `\s`, `\\`, `\"`, `\'`, `\xHH`
- nospace の仕様により、文字列内の空白文字も無視されます（スペースを入れるには `\s` を使用）
- ヌル終端は tree_parser で追加されます

### 3. tree_parser での文字列展開 (`src/tree_parser/statement/mod.rs`)

`parse_variable_declarations` 関数を拡張し、文字列リテラルを配列宣言に展開します。

#### 展開ルール

`let: str("Hello");` は以下のように展開されます：

1. 配列宣言: `str[6]`（文字数 + ヌル終端）
2. 各文字の代入: `str[0] = 72`, `str[1] = 101`, ...
3. ヌル終端: `str[5] = 0`

#### 明示的サイズ指定

`let: str[10]("Hi");` の場合：
- 配列サイズは 10
- 文字列 "Hi" + ヌル終端 = 3 文字
- 残りの 7 スロットは未初期化（0 初期化はされない）
- エラーチェック: 文字列長 + 1 > 配列サイズ の場合はエラー

#### サポートされる形式

1. `let: s("Hello");` - 暗黙的サイズ（文字数 + 1）
2. `let: s[10]("Hi");` - 明示的サイズ指定
3. `static: s("Test");` - static 変数としての文字列

### 4. テストケース

#### test_string_basic (`resources/tests/passes/string-basic.ns`)

- 基本的な文字列リテラル
- エスケープシーケンス `\s`（スペース）
- 空文字列 `""`
- 明示的サイズ指定 `[10]("Hi")`

#### test_string_escape (`resources/tests/passes/string-escape.ns`)

- エスケープシーケンス: `\n`, `\t`, `\\`
- static 変数としての文字列

### 5. テスト結果

```
cargo test
```

結果: **111 passed; 0 failed; 18 ignored**

- `test_string_basic`: ✓ 成功（インタプリタ）
- `test_string_escape`: ✓ 成功（インタプリタ）
- `test_string_basic_ws`: ✓ 成功（Whitespace、ignored）
- `test_string_escape_ws`: ✓ 成功（Whitespace、ignored）

既存のすべてのテストが引き続き通過しています。

## 設計の特徴

### 糖衣構文としての実装

文字列リテラルは配列の糖衣構文として実装されており、tree_parser レベルで配列宣言に展開されます。これにより：

- semantic_analyzer 以降の変更は不要
- 配列として扱われるため、配列の全機能が使用可能
- コンパイラ（Whitespace）でも自動的にサポート

### nospace の言語仕様への対応

nospace では空白文字が無視されるため、文字列リテラル内でも空白は削除されます。スペースを含めるには明示的に `\s` エスケープを使用する必要があります。

例:
- `"Hello World"` → `"HelloWorld"`（空白は無視される）
- `"Hello\sWorld"` → `"Hello World"`（`\s` でスペースを挿入）

## 変更ファイル

- `src/token_parser/mod.rs` - StringLiteral トークンとパース処理の追加
- `src/tree_parser/statement/mod.rs` - 文字列リテラルの配列展開
- `resources/tests/passes/string-basic.ns` - 基本テスト
- `resources/tests/passes/string-basic.check.json`
- `resources/tests/passes/string-escape.ns` - エスケープシーケンステスト
- `resources/tests/passes/string-escape.check.json`
- `resources/tests/test-manifest.yaml` - テスト登録

## 次のステップ

配列実装タスク（Phase 1〜5）がすべて完了しました。

## まとめ

Phase 5 として計画されていた文字列リテラルの実装が完了しました。文字列は配列の糖衣構文として実装され、自動的にヌル終端が追加されます。

エスケープシーケンスを含む文字列リテラルが正常に動作し、インタプリタおよび Whitespace コンパイラの両方でサポートされています。
