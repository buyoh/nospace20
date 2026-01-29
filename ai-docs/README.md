# ai-docs

AI Agent 向けのドキュメントディレクトリ。

## ディレクトリ構成

- [architecture/](architecture/README.md) - アーキテクチャに関するドキュメント
- [spec/](spec/README.md) - 設計・仕様に関するドキュメント
- [task/](task/README.md) - 現在作業中のタスク・進捗を記録

## プロジェクト概要

`nospace20` は、独自のプログラミング言語 `nospace` を解釈・実行するコンパイラ・インタプリタを開発するプロジェクトです。`nospace` は、コード中の任意の箇所のスペース、改行、タブを許容する、遊びを目的としたプログラミング言語です。

## クイックスタート

```bash
# ビルド
cargo build

# テスト実行
cargo test

# インタプリタ実行（標準入力からコードを読み込む）
echo 'func: main() { __clog(42); return: 0; }' | cargo run
```

## 言語仕様

言語 `nospace` 自体の仕様は、リポジトリルートの [spec.md](../spec.md) に記載されています。
