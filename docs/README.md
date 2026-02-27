# ドキュメント

nospace 言語に関するドキュメント群。

## ファイル一覧

| ファイル | 説明 |
|----------|------|
| [grammar.bnf](grammar.bnf) | nospace 言語の BNF 文法定義 |
| [bnf-validation.md](bnf-validation.md) | BNF の正当性検証ガイド |
| [profiler.md](profiler.md) | Whitespace VM プロファイラの使い方 |
| [optimize.md](optimize.md) | 最適化オプション (`--opt`) の説明 |

## 構文ハイライト

| ファイル | 説明 |
|----------|------|
| [../syntaxes/nospace.tmLanguage.json](../syntaxes/nospace.tmLanguage.json) | TextMate Grammar (VSCode等用) |

## ツール

| ファイル | 説明 |
|----------|------|
| [../tools/validate-grammar.sh](../tools/validate-grammar.sh) | BNF 検証スクリプト |
| [../tools/vscode-ext/](../tools/vscode-ext/) | VSCode 拡張ツール群 |
| [../tools/profile-report.py](../tools/profile-report.py) | プロファイル HTML レポート生成 |

```bash
# BNF 検証
./tools/validate-grammar.sh docs/grammar.bnf

# TextMate Grammar 検証
cd tools/vscode-ext && npm install && npm run validate

# VSCode 拡張ビルド
cd tools/vscode-ext && npm run build-ext
```

## 関連ドキュメント

| パス | 説明 |
|------|------|
| [../docs/spec.md](../docs/spec.md) | 言語仕様（正式） |
| [../docs/tutorial.md](../docs/tutorial.md) | チュートリアル |
| [../ai-docs/](../ai-docs/) | 開発者・AI向けドキュメント |
