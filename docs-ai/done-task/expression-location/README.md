# 式レベルの位置情報 (Phase 2)

`wasm-compile-error-location.md` Phase 2 の設計ドキュメント。

## 概要

`Expression` および `ExecExpression` に `SourceLocation` を付与し、コンパイルエラーの位置を式の精度で報告する。

Phase 1（文レベル位置情報）は完了済み。Phase 2 では式レベルに粒度を向上させる。

## ドキュメント一覧

- [overview.md](overview.md) - 全体設計・方針・作業順序
- [step1-tree-parser.md](step1-tree-parser.md) - Step 1: tree_parser への LocatedExpression 導入
- [step2-semantic-analyzer.md](step2-semantic-analyzer.md) - Step 2: semantic_analyzer への LocatedExecExpression 導入
- [step3-compiler-interpreter.md](step3-compiler-interpreter.md) - Step 3: compiler_ws / interpreter の対応
