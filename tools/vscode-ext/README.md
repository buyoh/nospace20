# VSCode Extension Tools

nospace 言語の VSCode 拡張を作成・検証するためのツール群。

## セットアップ

```bash
cd tools/vscode-ext
npm install
```

## 使用方法

### TextMate Grammar の検証

```bash
npm run validate

# または
node validate-tmgrammar.js ../../syntaxes/nospace.tmLanguage.json

# テストファイルでトークン化を確認
node validate-tmgrammar.js ../../syntaxes/nospace.tmLanguage.json ../../resources/tests/passes/c000.ns
```

### VSCode 拡張のビルド

```bash
npm run build-ext

# または
node build-vscode-ext.js
```

出力先: `dist/nospace-lang/`

### VSCode へのインストール

#### 開発モード

```bash
cp -r dist/nospace-lang ~/.vscode/extensions/
# VSCode を再起動
```

#### VSIX パッケージ

```bash
npm install -g @vscode/vsce
cd dist/nospace-lang
vsce package
```

生成された `.vsix` ファイルを VSCode でインストール。

## ファイル構成

```
tools/vscode-ext/
├── package.json           # npm 設定
├── validate-tmgrammar.js  # 文法検証ツール
├── build-vscode-ext.js    # 拡張ビルドツール
├── README.md              # このファイル
└── dist/                  # ビルド出力（gitignore）
    └── nospace-lang/
        ├── package.json
        ├── language-configuration.json
        ├── README.md
        └── syntaxes/
            └── nospace.tmLanguage.json
```
