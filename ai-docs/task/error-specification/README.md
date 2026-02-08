# nospace エラー仕様

nospace コンパイラ・インタプリタが検出・報告する全エラーの仕様をまとめるプロジェクト。

## 目的

- エラーメッセージの一貫性を保証する
- エラーの網羅性を確認し、テストカバレッジを向上させる
- ユーザー向けエラードキュメントの基礎資料とする
- ソースコードからエラー仕様を自動生成する手段を検討する

## エラーの分類

nospace のエラーは以下のフェーズで発生する：

1. **字句解析 (Token Parser)** - ソースコードをトークン列に変換する段階
2. **構文解析 (Tree Parser)** - トークン列を構文木に変換する段階
3. **意味解析 (Semantic Analyzer)** - 構文木の意味的妥当性を検証する段階
4. **コンパイル (Compiler)** - 意味解析済みのコードを Whitespace にコンパイルする段階
5. **実行時 (Runtime)** - Whitespace インタプリタの実行時エラー

## ドキュメント構成

- [token-parser-errors.md](token-parser-errors.md) - 字句解析エラー
- [tree-parser-errors.md](tree-parser-errors.md) - 構文解析エラー
- [semantic-errors.md](semantic-errors.md) - 意味解析エラー
- [compile-errors.md](compile-errors.md) - コンパイルエラー
- [runtime-errors.md](runtime-errors.md) - 実行時エラー
- [error-generation.md](error-generation.md) - エラー仕様の自動生成手段

## 現状のエラー実装

### エラー型の定義

| エラー型 | 定義場所 | 用途 |
|---------|---------|------|
| `CodeParseError` | `src/base/mod.rs` | 字句・構文・意味解析エラーの共通型 |
| `CompileError` | `src/compiler_ws/mod.rs` | Whitespace コンパイラのエラー |
| `ParseError` | `src/whitespace/parser.rs` | Whitespace パーサーのエラー |
| `RuntimeError` | `src/whitespace/interpreter.rs` | Whitespace 実行時エラー |

### テストケースの種類

test-manifest.yaml で定義されているエラーテスト：

- `syntax_error` - 字句・構文解析エラー (fails/syntax/ 配下)
- `compile_error` - 意味解析・コンパイルエラー (fails/compile/ 配下)
- その他スコープエラー (fails/scope/ 配下)

## タスク

- [ ] 各フェーズのエラーメッセージを網羅的に収集
- [ ] エラーメッセージのテストケース網羅性を検証
- [ ] 不足しているテストケースを特定
- [ ] エラー仕様の自動生成手段を検討・実装
- [ ] ユーザー向けエラーリファレンスの作成

## 備考

- エラーメッセージは静的文字列とフォーマット文字列の両方が使用されている
- デバッグビルド時は `CodeParseError` に呼び出し元の位置情報が記録される
- 一部のエラーは "弱い構文エラー" として報告され、パースを継続する
