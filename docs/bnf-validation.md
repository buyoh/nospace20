# BNF 検証ガイド

このドキュメントでは、`grammar.bnf` の正当性を検証するためのツールと評価方法を説明します。

## 概要

本プロジェクトでは以下のアプローチを採用します：

| 用途 | ファイル | 必須/オプション |
|------|----------|-----------------|
| 文法ドキュメント | `docs/grammar.bnf` | 必須（人間用） |
| BNF検証 | `tools/validate-grammar.sh` | 必須（CI用） |
| コンパイル時検証 | `grammar.pest` | オプション |
| 構文ハイライト | `syntaxes/nospace.tmLanguage.json` | オプション |

## 1. grammar.bnf の検証

### 1.1 検証スクリプト

`tools/validate-grammar.sh` で BNF ファイルの基本的な整合性を検証します。

**検証内容**:
- 構文的な整合性（`::=` の形式）
- 未定義の非終端記号の検出
- 未使用の規則の検出
- 既存パーサーテストとの整合性

**使用方法**:
```bash
./tools/validate-grammar.sh docs/grammar.bnf
```

### 1.2 Bison による競合検出（オプション）

LALR(1) 競合を検出したい場合は Bison を使用します。

```bash
# インストール
sudo apt install bison

# Yacc形式に変換して検証
bison -v grammar.y
cat grammar.output  # 競合レポート
```

## 2. pest によるコンパイル時検証（オプション）

### 2.1 有効化方法

`Cargo.toml` に feature フラグを追加：

```toml
[features]
default = []
grammar-check = ["pest", "pest_derive"]

[dependencies]
pest = { version = "2.7", optional = true }
pest_derive = { version = "2.7", optional = true }
```

### 2.2 使用方法

```bash
# grammar.pest を検証しながらビルド
cargo build --features grammar-check

# CLIでのデバッグ
cargo install pest_debugger
pest_debugger src/grammar.pest
```

### 2.3 pest 文法ファイル

`grammar.bnf` と同等の `grammar.pest` を作成します（必要な場合のみ）。

```pest
// src/grammar.pest
program = { SOI ~ (func | global_stmt)* ~ EOI }

integer = @{ ASCII_DIGIT+ }
ident = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }

expr = { expr_assign }
expr_assign = { expr_or ~ ("=" ~ expr_assign)? }
// ... 以下省略
```

## 3. TextMate Grammar（構文ハイライト）

### 3.1 概要

TextMate Grammar は VSCode 等のエディタで構文ハイライトに使用される JSON/YAML 形式の文法定義です。

**特徴**:
- Oniguruma 正規表現ベース
- スコープ（`keyword.control`, `string.quoted` など）でトークンを分類
- 多くのエディタ（VSCode, Sublime Text, Atom など）で共通

### 3.2 ファイル構造

```
.vscode-extension/           # VSCode拡張として配布する場合
├── package.json
└── syntaxes/
    └── nospace.tmLanguage.json

# または単体で配置
syntaxes/
└── nospace.tmLanguage.json
```

### 3.3 基本的な TextMate Grammar

`syntaxes/nospace.tmLanguage.json`:

```json
{
  "$schema": "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  "name": "nospace",
  "scopeName": "source.nospace",
  "patterns": [
    { "include": "#comments" },
    { "include": "#keywords" },
    { "include": "#strings" },
    { "include": "#numbers" },
    { "include": "#operators" },
    { "include": "#functions" },
    { "include": "#variables" }
  ],
  "repository": {
    "comments": {
      "name": "comment.block.nospace",
      "begin": "#",
      "end": "#"
    },
    "keywords": {
      "patterns": [
        {
          "name": "keyword.control.nospace",
          "match": "\\b(if|else|while|return|break|continue|func|let)\\b"
        }
      ]
    },
    "strings": {
      "name": "string.quoted.single.nospace",
      "begin": "'",
      "end": "'",
      "patterns": [
        {
          "name": "constant.character.escape.nospace",
          "match": "\\\\[\\\\tns'r]"
        }
      ]
    },
    "numbers": {
      "name": "constant.numeric.nospace",
      "match": "\\b[0-9]+\\b"
    },
    "operators": {
      "name": "keyword.operator.nospace",
      "match": "(==|!=|<=|>=|&&|\\|\\||[+\\-*/%<>=!])"
    },
    "functions": {
      "patterns": [
        {
          "name": "support.function.builtin.nospace",
          "match": "\\b(__clog|__assert|__assert_not|__trace|__puti|__putc|__geti|__getc)\\b"
        },
        {
          "name": "entity.name.function.nospace",
          "match": "\\b([a-zA-Z_][a-zA-Z0-9_]*)\\s*(?=\\()"
        }
      ]
    },
    "variables": {
      "name": "variable.other.nospace",
      "match": "\\b[a-zA-Z_][a-zA-Z0-9_]*\\b"
    }
  }
}
```

### 3.4 検証方法

**VSCode 内蔵ツール**:
```
コマンドパレット > Developer: Inspect Editor Tokens and Scopes
```

**CLI での検証**:
```bash
# vscode-textmate を使用
npm install -g vscode-textmate

# または、js-yaml で YAML から JSON に変換
npm install -g js-yaml
js-yaml syntaxes/nospace.tmLanguage.yaml > syntaxes/nospace.tmLanguage.json
```

**Yeoman によるスキャフォールド**:
```bash
npm install -g yo generator-code
yo code  # "New Language Support" を選択
```

### 3.5 作成難易度

| 項目 | 難易度 | 理由 |
|------|--------|------|
| 基本的なハイライト | ★☆☆ | 正規表現ベースで直感的 |
| 入れ子構造 | ★★☆ | begin/end パターンで対応可能 |
| 完全な構文チェック | ★★★ | TextMate では限界あり（パーサーではない）|

**結論**: 基本的な構文ハイライトは容易に作成できます。ただし、TextMate Grammar はトークン分類のみで、構文エラー検出には使えません。

## 4. CLI ツール（参考）

以下は文法検証に使用できる CLI ツールです。本プロジェクトでは `validate-grammar.sh` を推奨しますが、より本格的な検証が必要な場合に参照してください。

| ツール | 形式 | インストール | 用途 |
|--------|------|--------------|------|
| pest_debugger | PEG (.pest) | `cargo install pest_debugger` | インタラクティブデバッグ |
| tree-sitter | JS/JSON | `cargo install tree-sitter-cli` | パーサー生成・テスト |
| Bison | Yacc (.y) | `apt install bison` | LALR競合検出 |

## 5. Rust ライブラリ（参考）

コンパイル時検証やパーサー置き換えを検討する場合の選択肢です。

| ライブラリ | 特徴 | 推奨度 |
|------------|------|--------|
| pest | PEG ベース。コンパイル時検証 | ★★★ |
| lalrpop | BNF風の記法。LALR(1) | ★★★ |
| nom | パーサーコンビネータ | ★★ |
| chumsky | エラーリカバリが優秀 | ★★ |

## 6. 手動検証チェックリスト

### 6.1 左再帰のチェック

```bnf
# 左再帰（問題あり）
expr ::= expr "+" term

# 右再帰（問題なし）
expr ::= term "+" expr
```

現在の `grammar.bnf` は右再帰を使用しているため問題ありません。

### 6.2 曖昧性のチェック

- ダングリングelse問題
- 演算子の優先順位
- 結合性（左結合/右結合）

### 6.3 既存パーサーとの整合性

```bash
# すべてのテストを実行
cargo test

# パース結果を確認
cargo run -- parse resources/tests/passes/*.ns
```

## 参考リンク

- [pest Book](https://pest.rs/book/)
- [VSCode Syntax Highlight Guide](https://code.visualstudio.com/api/language-extensions/syntax-highlight-guide)
- [TextMate Language Grammars](https://macromates.com/manual/en/language_grammars)
