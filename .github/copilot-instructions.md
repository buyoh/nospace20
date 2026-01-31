# 概要

独自のプログラミング言語 `nospace` を解釈・実行するコンパイラ・インタプリタを開発するプロジェクト。

## ドキュメント

Agent 向けのドキュメントは `ai-docs/` ディレクトリに配置される。このディレクトリのファイルは指示なく書き換えて良い
各サブディレクトリには `README.md` ファイルがあり、インデックスとなっている

- `ai-docs/architecture/` : アーキテクチャに関する
- `ai-docs/spec/` : 設計・仕様に関する
- `ai-docs/task/` : 現在作業中のタスク・進捗を記録する

## テストについて

- テストは「Unitテスト」「largeテスト」の2種類がある

## Git について

コミットには以下のアカウントを設定してね  
同時に別の Agent が修正を行っているかもしれないため、修正するファイルだけをコミットしてね

- name: buyoh(agent)
- email: 15198247+buyoh@users.noreply.github.com

## SKILL について

以下に関連するタスクの場合、該当のドキュメントを参照してください。

`.github/skills/add-test-spec/SKILL.md` : `/resources/tests/` 以下にテストケースを追加するときに使う
`.github/skills/design-architecture/SKILL.md` : 仕様を基にソフトウェアの設計・コード変更方法を検討するときに使う
`.github/skills/update-code/SKILL.md` : .rs などアプリケーションに使用されるコードを更新する際に使う
