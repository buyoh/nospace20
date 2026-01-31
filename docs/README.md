# ドキュメント

nospace 言語に関するドキュメント群。

## ファイル一覧

| ファイル | 説明 |
|----------|------|
| [grammar.bnf](grammar.bnf) | nospace 言語の BNF 文法定義 |
| [bnf-validation.md](bnf-validation.md) | BNF の正当性検証ガイド |

## 構文ハイライト

| ファイル | 説明 |
|----------|------|
| [../syntaxes/nospace.tmLanguage.json](../syntaxes/nospace.tmLanguage.json) | TextMate Grammar (VSCode等用) |

## ツール

| ファイル | 説明 |
|----------|------|
| [../tools/validate-grammar.sh](../tools/validate-grammar.sh) | BNF 検証スクリプト |
| [../tools/vscode-ext/](../tools/vscode-ext/) | VSCode 拡張ツール群 |

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
| [../spec.md](../spec.md) | 言語仕様（正式） |
| [../tutorial.md](../tutorial.md) | チュートリアル |
| [../ai-docs/](../ai-docs/) | 開発者・AI向けドキュメント |
