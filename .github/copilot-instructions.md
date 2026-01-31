# 概要

独自のプログラミング言語 `nospace` を解釈・実行するコンパイラ・インタプリタを開発するプロジェクト。

## ドキュメント

プログラミング言語仕様は以下に配置される:

- `spec.md` : nospace 言語仕様
- `tutorial.md` : 簡単なチュートリアル

ソフトウェアの使い方については、`--help` オプションで表示される。

Agent 向けのドキュメントは `ai-docs/` ディレクトリに配置される。このディレクトリのファイルは指示なく書き換えて良い
各サブディレクトリには `README.md` ファイルがあり、インデックスとなっている

- `ai-docs/architecture/` : アーキテクチャに関する
- `ai-docs/spec/` : 設計・仕様に関する
- `ai-docs/task/` : 現在作業中のタスク・進捗を記録する

何らかの理由で指示通りの作業を行わなかったとき、その時点で必ず進捗として記録すること。

## テストについて

- テストは「Unitテスト」「largeテスト」の2種類がある
- 動作確認等のため一時ディレクトリ・ファイルが必要なときは、`.gitignore` に `/tmp` が追加されているため、ここに作成する

## Git について

コミットには以下のアカウントを設定してね  
同時に別の Agent が修正を行っているかもしれないため、修正するファイルだけをコミットしてね

- name: buyoh
- email: 15198247+buyoh@users.noreply.github.com

## SKILL について

以下に関連するタスクの場合、該当のドキュメントを参照してください。

`.github/skills/add-test-spec/SKILL.md` : `/resources/tests/` 以下にテストケースを追加するときに使う
`.github/skills/design-architecture/SKILL.md` : 仕様を基にソフトウェアの設計・コード変更方法を検討するときに使う
`.github/skills/update-code/SKILL.md` : .rs などアプリケーションに使用されるコードを更新する際に使う
