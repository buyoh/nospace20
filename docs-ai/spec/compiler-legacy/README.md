# nospace → Whitespace コンパイラ（旧実装）調査

このディレクトリには、旧実装（`.local/nospace/main.cpp`）における nospace から Whitespace への変換ロジックの調査・分析結果を記載しています。

## 目的

現在の nospace20 では Whitespace コンパイラが実装済みですが、旧実装の変換方式を参考資料として文書化しています。

## ドキュメント一覧

### 概要・基礎

| ファイル | 内容 |
|----------|------|
| [overview.md](overview.md) | コンパイラの全体像と変換フロー |
| [instructions.md](instructions.md) | Whitespace 命令セットのエンコーディング |
| [memory-layout.md](memory-layout.md) | メモリレイアウトとスタックフレーム管理 |

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

## 関連ドキュメント

- [../compiler-rust-impl/](../compiler-rust-impl/) - Rust 実装設計ドキュメント（現在の nospace20 の設計）
- [../compiler-test-strategy.md](../compiler-test-strategy.md) - コンパイラテスト戦略
- `src/compiler_ws/` - 実際の Rust 実装
