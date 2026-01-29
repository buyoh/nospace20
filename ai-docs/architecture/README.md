# Architecture

nospace20 のアーキテクチャに関するドキュメント。

## 目次

- [overview.md](overview.md) - システム全体の概要
- [modules.md](modules.md) - モジュール詳細

## 処理フロー概要

```
ソースコード (String)
     │
     ▼
┌─────────────────────┐
│   Token Parser      │  文字列 → トークン列
│   (token_parser)    │
└─────────────────────┘
     │
     ▼
┌─────────────────────┐
│   Tree Parser       │  トークン列 → AST (抽象構文木)
│   (tree_parser)     │
└─────────────────────┘
     │
     ▼
┌─────────────────────┐
│ Syntactic Analyzer  │  AST → 実行可能な構造体 (Scope)
│(syntactic_analyzer) │
└─────────────────────┘
     │
     ├────────────────────────────┐
     ▼                            ▼
┌─────────────────────┐    ┌─────────────────────┐
│   Interpreter       │    │     Compiler        │
│   (interpreter)     │    │    (compiler)       │
└─────────────────────┘    └─────────────────────┘
   直接実行                  (未実装) コード生成
```
