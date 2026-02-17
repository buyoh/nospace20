# 実装状況

## 概要

このプロジェクトは開発途中であり、多くの機能が未実装または部分的な実装状態です。

## モジュール別状況

| モジュール | 状況 | 備考 |
|------------|------|------|
| token_parser | ✅ 実装済み | 基本機能は動作 |
| tree_parser | ✅ 実装済み | 基本機能は動作 |
| semantic_analyzer | ⚠️ 部分実装 | エラーハンドリングが不完全 |
| interpreter | ✅ 実装済み | 基本機能は動作 |
| compiler_ws | ⚠️ 部分実装 | main関数のみ対応、ユーザー定義関数呼び出し・break・continue未実装 |
| logger | ✅ 実装済み | - |

## 言語機能別状況

**注**: 以下の表は interpreter の実装状況を示しています。compiler_ws (Whitespace コンパイラ) の実装状況は次節を参照してください。

| 機能 | 状況 | 備考 |
|------|------|------|
| 四則演算 | ✅ | `+`, `-`, `*`, `/` |
| 比較演算 | ✅ | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| 単項演算 | ✅ | `-` (負号), `!` (論理NOT) |
| 剰余演算 | ✅ | `%` |
| 論理演算 | ✅ | `&&`, `||`, `!` (短絡評価対応) |
| 変数定義 | ✅ | `let: x;` |
| 変数代入 | ✅ | `x = value;` |
| 配列 | ✅ | 宣言、アクセス、代入 |
| ポインタ | ✅ | `&` (参照), `*` (参照解除) |
| 関数定義 | ✅ | `func: name(args) { ... }` |
| 関数呼び出し | ✅ | `name(args)` |
| if 文 | ✅ | `if: cond { } else { };` |
| while 文 | ✅ | `while: cond { };` |
| return | ✅ | `return: expr;` |
| break | ✅ | `break;` |
| continue | ✅ | `continue;` |
| ブロックスコープ | ✅ | `{ ... }` |
| コメント | ✅ | `# comment #` |
| 文字リテラル | ✅ | `'A'`, `'\n'` 等 |
| 16進数リテラル | ✅ | `0xFF` |
| 標準入出力 | ✅ | `__puti`, `__putc`, `__geti`, `__getc` |
| デバッグビルトイン | ✅ | `__clog`, `__trace`, `__assert`, `__assert_not` |
| 型システム | ❌ | 未実装 |
| final/const | ❌ | 未実装 |
| 変数初期値 | ❌ | 未実装 |
| static 変数 | ✅ | Phase 4 で実装済み |

### Whitespace コンパイラ (compiler_ws) の機能状況

| 機能 | 状況 | 備考 |
|------|------|------|
| 四則演算 | ✅ | `+`, `-`, `*`, `/` |
| 比較演算 | ✅ | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| 単項演算 | ✅ | `-` (負号), `!` (論理NOT) |
| 剰余演算 | ✅ | `%` |
| 論理演算 | ✅ | `&&`, `||`, `!` (短絡評価なし、組み込みルーチン実装) |
| 変数定義 | ✅ | グローバル・ローカル変数 |
| 変数代入 | ✅ | `x = value;` |
| 配列 | ✅ | 宣言、アクセス、代入 (Phase 4) |
| ポインタ | ✅ | `&` (参照), `*` (参照解除) |
| 関数定義 | ⚠️ | main 関数のみ対応 |
| 関数呼び出し | ❌ | ユーザー定義関数呼び出し未実装 |
| if 式 | ✅ | `if: cond { } else { };` |
| while 式 | ✅ | `while: cond { };` |
| return | ✅ | `return: expr;` |
| break | ❌ | 未実装 |
| continue | ❌ | 未実装 |
| ブロックスコープ | ✅ | `{ ... }` |
| 標準入出力 | ✅ | `__puti`, `__putc`, `__geti`, `__getc` |
| デバッグビルトイン | ✅ | 引数のみ評価して無視 |

詳細は [../task/compiler/whitespace-missing-features.md](../task/compiler/whitespace-missing-features.md) を参照してください。

## 既知の問題

### semantic_analyzer

- 重複した識別子定義でパニックする (エラー回復なし)
- `Expression::Invalid` の処理が `todo!()` で未実装

### tree_parser

- 一部のエラーで `match_expect_token_unused!` を使用しており、エラーが無視される場合がある

### interpreter

- (なし)

### compiler_ws (Whitespace コンパイラ)

- ユーザー定義関数呼び出しが未実装（main 関数のみ対応）
- break 文が未実装
- continue 文が未実装
- 短絡評価が実装されていない（論理演算は組み込みルーチンで実装）
- 配列の境界チェックなし（Whitespace の命令セット制約）

## TODO リスト

### コードベース全般

1. `Clone` derive の削除 (パフォーマンス最適化)
2. `CodeParseError.message` を `Cow<'static, str>` に変更検討

### compiler_ws (Whitespace コンパイラ)

Phase 7: ユーザー定義関数呼び出しのサポート
- 複数関数の定義生成 (現在は main のみ)
- 関数呼び出しのコード生成
- テストケースの追加

Phase 8: break/continue のサポート
- `CodeGenContext` にループラベルスタックを追加
- while 式生成時にラベルをプッシュ/ポップ
- break/continue 文のコード生成
- ネストしたループのテスト

詳細は [../task/compiler/whitespace-missing-features.md](../task/compiler/whitespace-missing-features.md) を参照してください。
