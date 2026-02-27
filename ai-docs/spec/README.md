# Spec

設計・仕様に関するドキュメント。

## 目次

- [implementation-status.md](implementation-status.md) - 実装状況
- [compiler-legacy/](compiler-legacy/) - Whitespace コンパイラ（旧実装）の調査ドキュメント
- [compiler-rust-impl/](compiler-rust-impl/) - Whitespace コンパイラ（現在の実装）の設計ドキュメント
- [compiler-test-strategy.md](compiler-test-strategy.md) - Whitespace コンパイラのテスト戦略
- [elsif-keyword.md](elsif-keyword.md) - elsif キーワード仕様（構文・BNF・セマンティクス・廃止事項）

## 言語仕様

言語 nospace の仕様は、リポジトリルートの [docs/spec.md](../../docs/spec.md) を参照してください。

docs/spec.md には以下の内容が含まれています:
- リテラル・識別子（数値、識別子、コメント）
- 演算（四則演算、単項演算子、比較演算子、演算子の優先順位）
- 組み込み識別子（`__clog`, `__assert`, `__trace` など）
- 代入・変数定義
- 関数定義
- 制御構文（while, if, break, continue, return など）
- スコープ
- 型（未実装）
- 既知の制限事項
