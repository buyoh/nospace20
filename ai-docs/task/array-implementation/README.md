# 配列実装タスク

spec.md §4.2「配列」および §4.3「文字列」の実装計画。

## 概要

nospace 言語に固定長配列を導入する。配列はスタック上に連続スロットとして確保され、
既存の変数管理・参照システムを自然に拡張する。文字列は配列の糖衣構文として実装する。

## ドキュメント一覧

- [overview.md](overview.md) - 全体設計概要・方針・フェーズ分割
- [phase1-tree-parser.md](phase1-tree-parser.md) - Phase 1: 構文解析（tree_parser）の変更
- [phase2-semantic-analyzer.md](phase2-semantic-analyzer.md) - Phase 2: 意味解析（semantic_analyzer）の変更
- [phase3-interpreter.md](phase3-interpreter.md) - Phase 3: インタプリタ（interpreter）の変更
- [phase4-compiler-ws.md](phase4-compiler-ws.md) - Phase 4: Whitespace コンパイラ（compiler_ws）の変更
- [phase5-string-literal.md](phase5-string-literal.md) - Phase 5: 文字列リテラル（糖衣構文）の実装

## 前提条件

- token_parser は変更不要（`BracketL` / `BracketR` は既に定義済み）
- 各フェーズは順序依存だが、Phase 4 と Phase 5 は独立して進行可能

## 現在の状態

- [x] Phase 1: 構文解析 - 完了 (2026-02-10) - [レポート](../../done-task/phase1-implementation-report.md)
- [x] Phase 2: 意味解析 - 完了 (2026-02-10) - [レポート](../../done-task/phase2-implementation-report.md)
- [x] Phase 3: インタプリタ - 完了 (2026-02-10)
- [x] Phase 4: Whitespace コンパイラ - 完了 (2026-02-13) - [レポート](../../done-task/phase4-implementation-report.md)
- [ ] Phase 5: 文字列リテラル - 未実装
