# nospace20 テストケース

このディレクトリには、nospace20 のテストケースが格納されています。テストケースは `test-manifest.yaml` で定義され、`build.rs` によって自動的にテストコードに変換されます。

## ディレクトリ構造

```
resources/tests/
├── test-manifest.yaml    # テスト定義ファイル
├── passes/               # 成功するテストケース
│   ├── *.ns              # nospace ソースファイル
│   └── *.check.json      # 期待する実行結果
└── fails/                # 失敗するテストケース
    ├── syntax/           # 構文エラーのテスト
    ├── semantic/         # セマンティクスエラーのテスト
    └── runtime/          # 実行時エラーのテスト
```

## テストの種類

### 1. `success` テスト

正常に実行され、`__trace()` を使って実行フローを検証するテスト。

**ファイル構成:**
- `xxx.ns`: nospace ソースコード
- `xxx.check.json`: 期待する trace 結果

**例:** [passes/c000.ns](passes/c000.ns)

```nospace
func: main() {
  __trace(0);
}
```

**期待結果:** [passes/c000.check.json](passes/c000.check.json)

```json
{
  "trace": [1]
}
```

**`trace` の意味:**
- 配列の各要素は、対応するトレースポイント (0, 1, 2, ...) の実行回数を表します
- `"trace": [2, 1, 1, 1]` は、`__trace(0)` が2回、`__trace(1)` が1回、`__trace(2)` が1回、`__trace(3)` が1回実行されたことを意味します

### 2. `success_io` テスト

正常に実行され、標準入出力の内容を検証するテスト。

**ファイル構成:**
- `xxx.ns`: nospace ソースコード
- `xxx.check.json`: 期待する入出力結果

**例:** [passes/io/puti_basic_001.ns](passes/io/puti_basic_001.ns)

```nospace
func: main() {
    __puti(42);
    __puti(0);
}
```

**期待結果:** [passes/io/puti_basic_001.check.json](passes/io/puti_basic_001.check.json)

```json
{
  "type": "success_io",
  "stdout": "420"
}
```

**フィールド:**
- `type`: `"success_io"` を指定
- `stdout`: (オプション) 期待する標準出力の内容
- `stdin`: (オプション) 標準入力として与えるデータ
- `stdout_file`: (オプション) 期待する標準出力をファイルから読み込む
- `stdin_file`: (オプション) 標準入力をファイルから読み込む
- `cases`: (オプション) 複数のテストケースを定義する配列（後述）

#### 複数の入出力ケース

1つのテストに複数の入出力パターンを定義できます。各ケースで異なる入力を与えて、同じプログラムの動作を複数のパターンで検証できます。

**例:** [passes/io/geti_multiple_cases.check.json](passes/io/geti_multiple_cases.check.json)

```json
{
  "type": "success_io",
  "cases": [
    {
      "name": "positive",
      "stdin": "42\n",
      "stdout": "42"
    },
    {
      "name": "zero",
      "stdin": "0\n",
      "stdout": "0"
    },
    {
      "name": "negative",
      "stdin": "-100\n",
      "stdout": "-100"
    }
  ]
}
```

**`cases` 配列のフィールド:**
- `name`: (オプション) ケースの識別名（テスト失敗時のメッセージに使用）
- `stdin`: (オプション) 標準入力として与えるデータ
- `stdin_file`: (オプション) 標準入力をファイルから読み込む
- `stdout`: (オプション) 期待する標準出力の内容
- `stdout_file`: (オプション) 期待する標準出力をファイルから読み込む

**後方互換性:** `cases` を使わない従来の形式も引き続きサポートされます。

### 3. `syntax_error` テスト

構文解析時にエラーが発生することを検証するテスト。

**ファイル構成:**
- `xxx.ns`: 構文エラーを含む nospace ソースコード
- `xxx.check.json`: 期待するエラー情報

**例:** [fails/syntax/invalid_token_001.ns](fails/syntax/invalid_token_001.ns)

```nospace
func: main() {
  @ invalid token
}
```

**期待結果:** [fails/syntax/invalid_token_001.check.json](fails/syntax/invalid_token_001.check.json)

```json
{
  "type": "parse_error",
  "phase": "tokenize"
}
```

**フィールド:**
- `type`: `"parse_error"` を指定
- `phase`: エラーが発生したフェーズ (`"tokenize"` または `"parse"`)

## test-manifest.yaml の書き方

`test-manifest.yaml` でテストを定義します:

```yaml
tests:
  - name: test_xxx_001           # テスト関数名 (Rust のテスト関数として生成される)
    type: success                # テストの種類: success | success_io | syntax_error
    path: xxx/xxx_001            # テストファイルのパス (拡張子なし、passes/ または fails/ からの相対パス)
    comment: "Test description"  # (オプション) テストの説明
```

### フィールド

- **name**: Rust のテスト関数名として使用されます。`test_` で始めることを推奨します。
- **type**: テストの種類
  - `success`: 正常実行、trace チェック
  - `success_io`: 正常実行、標準入出力チェック
  - `syntax_error`: 構文エラー検証
- **path**: テストファイルのパス
  - `success` / `success_io` の場合: `passes/` からの相対パス
  - `syntax_error` の場合: `fails/syntax/` からの相対パス
  - 拡張子 `.ns` は省略します
- **comment**: テストの説明 (省略可)

### テストの無効化

テストを一時的に無効化するには、該当部分をコメントアウトします:

```yaml
# DISABLED: hangs (break/continue issue)
# - name: test_ok_coding_c002
#   type: success
#   path: c002
```

## テストケースの追加方法

1. **テストファイルを作成**
   - 成功テスト: `resources/tests/passes/` 以下に配置
   - 失敗テスト: `resources/tests/fails/syntax/` 以下に配置
   - ファイル名: `カテゴリ/テスト名.ns` (例: `operators/arith_001.ns`)

2. **期待結果ファイルを作成**
   - 同じ場所に `テスト名.check.json` を配置
   - JSON形式で期待する結果を記述

3. **test-manifest.yaml に追加**
   - 適切なカテゴリのセクションに追加
   - `name`, `type`, `path` を指定

4. **ビルドして確認**
   ```bash
   cargo test
   ```

テストケースは `build.rs` により自動生成されるため、`test-manifest.yaml` を編集後にビルドが必要です。

## カテゴリの整理

テストは以下のカテゴリに分類されています:

- `literals/`: リテラル (数値、文字、コメントなど)
- `operators/`: 演算子 (算術、比較、論理など)
- `variables/`: 変数
- `control_flow/`: 制御構文 (if, while, break, continue, return)
- `functions/`: 関数
- `builtins/`: 組み込み関数 (`__trace`, `__assert`)
- `io/`: 入出力関数 (`__puti`, `__putc`, `__geti`, `__getc`)
- `scope/`: スコープ
- `integration/`: 統合テスト
- `legacy/`: レガシーテスト (後方互換性のため残されているもの)

新しいテストを追加する際は、適切なカテゴリに配置してください。

## 関連ツール

- `tools/addtest.sh`: テストケースを簡単に追加するためのシェルスクリプト
