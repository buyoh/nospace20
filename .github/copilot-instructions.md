# 概要

独自のプログラミング言語 `nospace` を解釈・実行するコンパイラ・インタプリタを開発するプロジェクト

## ドキュメント

プログラミング言語仕様は以下に配置:

- `docs/spec.md` : nospace 言語仕様
- `docs/tutorial.md` : 簡単なチュートリアル

ソフトウェアの使い方については、`--help` オプションで確認

Agent 向けのドキュメントは `ai-docs/` ディレクトリに配置。このディレクトリのファイルは指示なく書き換えて良い
各サブディレクトリには `README.md` ファイルがあり、インデックスとなっている

- `ai-docs/architecture/` : アーキテクチャに関する
- `ai-docs/spec/` : 設計・仕様に関する
- `ai-docs/task/` : 現在作業中のタスク・進捗を記録する
- `ai-docs/done-task/` : 完了したタスクのアーカイブ

何らかの理由で指示通りの作業を行わなかったとき、その時点で必ず進捗として記録

README.md には実行方法・ビルド方法が記載されている。これらを変更したとき、README.md も更新

## ディレクトリ構造

```
examples/          # Rust のサンプルコード (ws_profiler など)
resources/
  tests/           # nospace のlargeテストケース (test-manifest.yaml で定義)
    passes/        # 成功するテストケース (.ns + .check.json)
    fails/         # 失敗するテストケース (syntax/, semantic/, runtime/)
  tests_alloc/     # アロケータ関連のlargeテストケース
  tests_ws/        # Whitespace インタプリタ直接テスト (.wsa + .check.json)
src/
  bin/             # CLI バイナリ (nospace20, whitespace20)
  token_parser/    # トークナイザ (ソースコード → トークン列)
  tree_parser/     # 構文解析 (トークン列 → AST)
  semantic_analyzer/ # 意味解析 (型チェック、スコープ解決など)
  compiler_ws/     # Whitespace コードへのコンパイラ
  optimizer/       # Whitespace コードの最適化パス
  interpreter/     # Whitespace インタプリタ (実行エンジン)
  whitespace/      # Whitespace パーサ・インタプリタ (低レベル)
  base/            # 共通型・エラー定義・ユーティリティ
  algorithm/       # アルゴケータ仕様の実装
  logger/          # ログ出力
  wasm_api/        # WebAssembly API
  lib.rs           # ライブラリルート
  cli_utils.rs     # CLI ユーティリティ
  compile_property.rs # コンパイルオプション定義
src_build/         # build.rs から利用されるテスト生成コード
syntaxes/          # VS Code 用シンタックスハイライト定義 (tmLanguage)
tests/             # Rust の結合テスト (build.rs で自動生成されるテストを含む)
tools/
  ci/              # CI/CD スクリプト
  vscode-ext/      # VS Code 拡張のビルド・検証ツール
  wasm-test/       # WASM ビルドのテスト
  wsc-install/     # whitespacers (wsc) のインストール先
  profile-report.py  # プロファイルレポート生成
  setup-wsc.sh     # wsc セットアップスクリプト
```

## テストについて

- テストは「Unitテスト」「largeテスト」の2種類
- 動作確認等のため一時ディレクトリ・ファイルが必要なときは、`.gitignore` に `./tmp` が追加されているため、ここに作成
- `wsc` は `./tools/wsc-install/bin/wsc` にある

## コーディングについて

- 実際にリモートにアクセスしたり、ファイルを作成するようなテストの作成は禁止。必ずモックを使用
- Mock ライブラリ等によるメソッドの差し替えは禁止。スタブ・ドライバクラスや依存性注入・テンプレートを使用する
- 単一責任原則に従い、モジュール・構造体を分割する。例えば、設定ファイルの解析の場合、「ファイルの読み込み」「データのバリデーション」「データアクセス」を分類
- エラーハンドリングについて、ドメイン上の想定される失敗・は戻り値で返し、システム的な致命的欠陥・回復不能な異常のみ例外/Panicを使用
- 構造体の概要は必ずドキュメントコメントとして追加する。関数は規模が小さい場合、省略可
  - 構造体のフィールドには、型定義だけでは意味が不明な場合、ドキュメントコメントを追加する。
- 時間・長さに関する値は、単位を明示する。例えば、`timeoutMs`、`fileSizeBytes`、`minXPx` のようにする
- バグを修正する場合、ソースコード中にバグの発生原因と解決策をコメントとして記載
- バグを修正する場合、関連するテストケースを追加・修正
- Unitテストはモジュールごとに作成。自明なモジュールには不要
- テストが失敗した際、原因が今回の修正によるものである場合は修正をするが、別の原因だった場合にはテストは修正せず、Failのままにしておく
