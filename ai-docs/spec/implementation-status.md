# 実装状況

## 概要

このプロジェクトは開発途中であり、多くの機能が未実装または部分的な実装状態です。

## モジュール別状況

| モジュール | 状況 | 備考 |
|------------|------|------|
| token_parser | ✅ 実装済み | 基本機能は動作 |
| tree_parser | ✅ 実装済み | 基本機能は動作 |
| syntactic_analyzer | ⚠️ 部分実装 | エラーハンドリングが不完全 |
| interpreter | ✅ 実装済み | 基本機能は動作 |
| compiler | ❌ 未実装 | `todo!` のみ |
| logger | ✅ 実装済み | - |

## 言語機能別状況

| 機能 | 状況 | 備考 |
|------|------|------|
| 四則演算 | ✅ | `+`, `-`, `*`, `/` |
| 比較演算 | ✅ | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| 単項演算 | ✅ | `-` (負号), `!` (論理NOT) |
| 剰余演算 | ✅ | `%` |
| 論理演算 | ✅ | `&&`, `||`, `!` (短絡評価対応) |
| 変数定義 | ✅ | `let: x;` |
| 変数代入 | ✅ | `x = value;` |
| 関数定義 | ✅ | `func: name(args) { ... }` |
| 関数呼び出し | ✅ | `name(args)` |
| if 文 | ✅ | `if: cond { } else { };` |
| while 文 | ✅ | `while: cond { };` |
| return | ✅ | `return: expr;` |
| break | ✅ | `break;` |
| continue | ✅ | `continue;` |
| コメント | ✅ | `# comment #` |
| 文字リテラル | ✅ | `'A'`, `'\n'` 等 |
| 標準入出力 | ✅ | `__puti`, `__putc`, `__geti`, `__getc` |
| 型システム | ❌ | 未実装 |
| final/const | ❌ | 未実装 |
| 変数初期値 | ❌ | 未実装 |
| static 変数 | ❌ | 未実装 |

## 既知の問題

### syntactic_analyzer

- 重複した識別子定義でパニックする (エラー回復なし)
- `Expression::Invalid` の処理が `todo!()` で未実装

### tree_parser

- 一部のエラーで `match_expect_token_unused!` を使用しており、エラーが無視される場合がある

### interpreter

- if/while は式だが、常に 0 を返す (仕様通りだが将来改善予定)

## TODO リスト (コード内のコメントより)

1. `Clone` derive の削除 (パフォーマンス最適化)
2. `CodeParseError.message` を `Cow<'static, str>` に変更検討
3. 16進数リテラル (`0x`) のサポート
4. コンパイラの実装 (grayspace ターゲット)
