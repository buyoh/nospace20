# システム概要

## プロジェクト構造

```
nospace20/
├── src/
│   ├── lib.rs              # ライブラリエントリポイント
│   ├── base/               # 共通定義 (エラー型等)
│   ├── bin/                # 実行バイナリ
│   │   └── nospace20.rs    # CLIエントリポイント
│   ├── token_parser/       # 字句解析器
│   ├── tree_parser/        # 構文解析器
│   ├── semantic_analyzer/  # 意味解析器
│   ├── interpreter/        # インタプリタ
│   ├── compiler/           # コンパイラ (未実装)
│   └── logger/             # ログ・テキスト処理ユーティリティ
├── tests/
│   └── code_test.rs        # 統合テスト (large テスト)
├── resources/
│   └── tests/              # テストケース (.ns + .check.json)
└── docs/spec.md            # 言語仕様書
```

## 公開 API (lib.rs)

| 関数 | 入力 | 出力 | 説明 |
|------|------|------|------|
| `parse_to_tokens` | `&String` | `Result<Vec<PrettyToken>, Vec<CodeParseError>>` | 字句解析 |
| `parse_to_tree` | `&Vec<PrettyToken>` | `Result<Vec<Statement>, Vec<CodeParseError>>` | 構文解析 |
| `syntactic_analyze` | `&Vec<Statement>` | `Scope` | 意味解析 |
| `optimize` | `&mut Scope, &OptimizationOptions` | `()` | 最適化パスの適用 |
| `interpret` | `&Scope` | `Option<i64>` | グローバル変数初期化 + main 実行 |
| `interpret_func` | `&Scope, &str` | `Option<i64>` | 関数実行（グローバル変数初期化なし） |
| `interpret_func_testing` | `&Scope, &str` | `BTreeMap<i64, i64>` | テスト用関数実行 (トレース情報付き) |
| `interpret_func_with_io` | `&Scope, &str, &str` | `(BTreeMap<i64, i64>, String)` | I/O付きテスト用関数実行 |

## 処理パイプライン詳細

### 1. Token Parser (字句解析)

- **入力**: ソースコード文字列
- **出力**: トークン列 (`Vec<PrettyToken>`)
- **責務**: 
  - 空白・コメントの除去
  - キーワード・識別子・リテラル・演算子の認識

### 2. Tree Parser (構文解析)

- **入力**: トークン列
- **出力**: 抽象構文木 (`Vec<Statement>`)
- **責務**:
  - 文法に基づくトークン列の構造化
  - 式・文の構文エラー検出

### 3. Syntactic Analyzer (意味解析)

- **入力**: 抽象構文木
- **出力**: スコープ構造 (`Scope`)
- **責務**:
  - 変数・関数の識別子解決
  - スコープ構造の構築
  - 実行可能な中間表現への変換

### 3.5. Optimizer (最適化) - オプショナル

- **入力**: スコープ構造 (`&mut Scope`)
- **出力**: 最適化されたスコープ構造
- **責務**:
  - 定数畳み込み、条件式最適化などの最適化パスを適用
  - `OptimizationOptions` で各パスを個別に制御
- **CLI**: `--opt=1` で全最適化を有効化

### 4. Interpreter (インタプリタ)

- **入力**: スコープ構造、エントリ関数名
- **出力**: 実行結果
- **責務**:
  - コードの逐次実行
  - 変数の値管理
  - 組み込み関数の提供

### 5. Compiler (コンパイラ) - 未実装

- **入力**: スコープ構造
- **出力**: 実行コード
- **責務**:
  - 他の中間言語 (例: Whitespace) へのコンパイル

## テスト戦略

### Unit テスト

各モジュール内で `#[cfg(test)]` を使用して実装。

### Large テスト (統合テスト)

`tests/code_test.rs` で実装。テストケースは `resources/tests/` に配置。

テストケースのフォーマット:
- `*.ns` - nospace ソースコード
- `*.check.json` - 期待される結果 (`{ "trace": [expected_values] }`)

`__trace(n)` 組み込み関数でトレースポイントを記録し、実行後に期待値と比較する。
