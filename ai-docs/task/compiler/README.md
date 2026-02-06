# nospace → Whitespace コンパイラ調査

このディレクトリには、旧実装（`.local/nospace/main.cpp`）における nospace から Whitespace への変換ロジックの調査・分析結果を記載しています。

## 目的

現在の nospace20 実装はインタプリタのみですが、将来的に Whitespace へのコンパイル機能を実装する際の参考資料として、旧実装の変換方式を文書化しています。

## ドキュメント一覧

### 概要・基礎

| ファイル | 内容 |
|----------|------|
| [overview.md](overview.md) | コンパイラの全体像と変換フロー |
| [instructions.md](instructions.md) | Whitespace 命令セットのエンコーディング |
| [memory-layout.md](memory-layout.md) | メモリレイアウトとスタックフレーム管理 |

### Rust 実装設計

詳細な Rust 実装設計は [rust-impl/](rust-impl/) サブディレクトリに分割されています。

| ファイル | 内容 |
|----------|------|
| [rust-impl/README.md](rust-impl/README.md) | 実装設計ドキュメントのインデックス |
| [rust-impl/overview.md](rust-impl/overview.md) | 設計方針・モジュール構成 |
| [rust-impl/whitespace.md](rust-impl/whitespace.md) | 命令表現・プログラム構造 |
| [rust-impl/memory-label.md](rust-impl/memory-label.md) | メモリレイアウト・ラベル管理 |
| [rust-impl/codegen.md](rust-impl/codegen.md) | コード生成 |
| [rust-impl/builtin.md](rust-impl/builtin.md) | 組み込みルーチン |
| [rust-impl/api-cli.md](rust-impl/api-cli.md) | 公開API・CLI統合 |
| [rust-impl/implementation-plan.md](rust-impl/implementation-plan.md) | テスト戦略・実装計画 |

### 演算子

| ファイル | 内容 |
|----------|------|
| [arithmetic.md](arithmetic.md) | 算術演算子 (`+`, `-`, `*`, `/`, `%`) と比較演算子 (`==`, `!=`, `<`, `<=`, `>`, `>=`) |
| [logical.md](logical.md) | 論理演算子 (`&&`, `\|\|`, `!`) |
| [assignment.md](assignment.md) | 代入演算子 (`=`, `+=`, `-=`, `*=`, `/=`, `%=`) とポインタ操作 (`*`, `&`, `[]`) |

### 制御構造

| ファイル | 内容 |
|----------|------|
| [control-flow.md](control-flow.md) | 条件分岐 (`if`/`elsif`/`else`) とループ (`while`) |
| [functions.md](functions.md) | 関数定義・呼び出し・return 文 |

### 組み込み機能

| ファイル | 内容 |
|----------|------|
| [io.md](io.md) | 入出力関数 (`__puti`, `__putc`, `__geti`, `__getc`, `__getiv`, `__getcv`) |
| [builtin-routines.md](builtin-routines.md) | 比較・論理演算用の組み込みサブルーチン |
| [io-builtin-design.md](io-builtin-design.md) | I/O ビルトイン関数の実装設計 |
| [builtin-functions-todo.md](builtin-functions-todo.md) | ビルトイン関数の実装 TODO |

### テスト

| ファイル | 内容 |
|----------|------|
| [test-strategy.md](test-strategy.md) | コンパイラテスト戦略（wsc 連携） |

## 変換の概要

### 処理フロー

```
nospace ソースコード
    ↓ 字句解析 (Parser::parseToTokens)
トークン列
    ↓ 構文解析 (Compiler::getStatementsScope)
AST (抽象構文木)
    ↓ コード生成 (Builder::convert*)
Whitespace 命令列
    ↓ 出力
空白文字のみのコード
```

### メモリモデル

- **スタック**: 式の評価、関数引数、戻り値
- **ヒープ**: 変数格納、スコープ情報

### ラベル管理

- ラベル 0-15: 組み込みルーチン用
- ラベル 16+: ユーザーコード（関数、制御構造）

## 旧実装の特徴

### 実装されている機能

- 整数演算（四則演算、剰余）
- 比較演算子
- 論理演算子（短絡評価なし）
- 変数（グローバル・ローカル）
- 配列
- ポインタ（参照・参照解除）
- 関数（再帰対応）
- 制御構造（if/elsif/else、while）
- I/O（整数・文字の入出力）

### 制限事項

- 論理演算子は短絡評価を行わない
- ネストした関数定義は不可
- break/continue は未実装

## 参考

- 旧実装ソース: `.local/nospace/main.cpp`
- Whitespace 仕様: `spec-whitespace.md`
- nospace 言語仕様: `spec.md`
