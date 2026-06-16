# Whitespace環境構築の進捗

## 完了日時

2026-02-06

## 概要

nospace → Whitespace コンパイラのテスト環境を構築しました。whitespacers (wsc) を使用してコンパイル結果を実行・検証できるようになりました。

## 実施内容

### 1. .gitignore の更新

`/tools/wsc-install` をgitignoreに追加し、個別にインストールしたwscバイナリを除外しました。

### 2. wsc インストールスクリプトの作成

[tools/setup-wsc.sh](../../tools/setup-wsc.sh) を作成:
- `cargo install whitespacers --root ./tools/wsc-install` でインストール
- ローカルディレクトリにインストールすることでライセンス分離を明確化（MPL-2.0とMITの分離）

### 3. wsc-install ディレクトリのREADME作成

[tools/wsc-install/README.md](../../tools/wsc-install/README.md) を作成:
- インストール方法を記載
- ライセンス情報を明記

### 4. テストユーティリティの実装

[tests/common/mod.rs](../../tests/common/mod.rs) を作成:

主な機能:
- `find_wsc()` - wsc実行ファイルのパスを解決（プロジェクト内優先、グローバルフォールバック）
- `wsc_available()` - wscの利用可能性チェック
- `run_whitespace(ws_code, stdin_input)` - Whitespaceコードを実行して結果を取得

クロスプラットフォーム対応:
- Unix: `which` コマンドでグローバルwscを検索
- Windows: `where` コマンドでグローバルwscを検索
- Windows用 `.exe` 拡張子を考慮

### 5. 統合テストの追加

[tests/compile_test.rs](../../tests/compile_test.rs) に以下を追加:

- `require_wsc!()` マクロ - wsc未インストール時にテストをスキップ
- `#[ignore]` 属性 - デフォルトではスキップ（`cargo test -- --ignored` で実行）

追加されたテスト:
1. `test_compile_and_run_puti` - `__puti(42)` の出力確認
2. `test_compile_and_run_putc` - `__putc(65)` の文字出力確認
3. `test_compile_and_run_arithmetic` - 算術演算結果の出力確認
4. `test_compile_and_run_variable` - 変数経由の出力確認
5. `test_compile_and_run_geti` - 入力処理の確認

### 6. Cargo.toml の更新

dev-dependenciesに `tempfile = "3.0"` を追加:
- 一時ファイルを使ってWhitespaceコードを実行するために必要

### 7. wsc のインストール

WSL環境で以下を実行:
```bash
cd tools
./setup-wsc.sh
```

インストール結果:
- バイナリ: `tools/wsc-install/bin/wsc`
- サイズ: 約7.9MB
- バージョン: whitespacers v1.3.0

## テスト結果

### 実行コマンド
```bash
cargo test --test compile_test -- --ignored --nocapture
```

### 結果
すべてのテストが **失敗** しました。

### エラー内容
```
"Undefined function: __puti"
"Undefined function: __putc"
"Undefined function: __geti"
```

### 原因
ビルトイン関数（`__puti`, `__putc`, `__geti`, `__getc`）がコンパイラに実装されていません。

## 次のステップ

ビルトイン関数の実装が必要です。詳細は以下のTODOドキュメントに記載:

👉 [builtin-functions-todo.md](builtin-functions-todo.md)

実装が必要な関数:
- `__puti(value: int)` - 整数出力 (OutputNumber命令)
- `__putc(value: int)` - 文字出力 (OutputChar命令)
- `__geti() -> int` - 整数入力 (InputNumber命令)
- `__getc() -> int` - 文字入力 (InputChar命令)

## 使用方法

### 開発者向けセットアップ

```bash
# 1. wsc をインストール
./tools/setup-wsc.sh

# 2. 通常のテスト（wsc不要なテストのみ）
cargo test --test compile_test

# 3. wsc統合テストを含むすべてのテスト
cargo test --test compile_test -- --ignored
```

### wsc の手動実行

```bash
# Whitespace ファイルを実行
./tools/wsc-install/bin/wsc path/to/file.ws

# 標準入力から入力を与える
echo "42" | ./tools/wsc-install/bin/wsc path/to/file.ws

# ヘルプ
./tools/wsc-install/bin/wsc --help
```

## ファイル一覧

### 新規作成
- [tools/setup-wsc.sh](../../tools/setup-wsc.sh)
- [tools/wsc-install/README.md](../../tools/wsc-install/README.md)
- [tests/common/mod.rs](../../tests/common/mod.rs)
- [docs-ai/task/compiler/builtin-functions-todo.md](builtin-functions-todo.md)

### 変更
- [.gitignore](../../.gitignore) - `/tools/wsc-install` を追加
- [Cargo.toml](../../Cargo.toml) - `tempfile` dev-dependencyを追加
- [tests/compile_test.rs](../../tests/compile_test.rs) - wsc統合テストを追加

## 参考資料

- [whitespacers (crates.io)](https://crates.io/crates/whitespacers)
- [whitespacers (GitHub)](https://github.com/CensoredUsername/whitespace-rs)
- [docs/spec-whitespace.md](../../docs/spec-whitespace.md)
- [test-strategy.md](test-strategy.md)
