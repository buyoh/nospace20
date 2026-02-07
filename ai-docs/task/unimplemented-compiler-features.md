# 未実装のコンパイラ機能

このドキュメントは nospace プログラミング言語における未実装のコンパイラ機能をまとめたものです。

最終更新日: 2026-02-07

## 目次

1. [コンパイラ全般](#1-コンパイラ全般)
2. [grayspace ターゲット](#2-grayspace-ターゲット)

---

## 1. コンパイラ全般

**状態**: ❌ 未実装

**説明**: コンパイラモジュールは完全に未実装。現在はインタプリタのみ動作。

**ファイル**: [src/compiler/mod.rs](../../src/compiler/mod.rs)

**内容**: 
```rust
// todo!
```

**目的**: nospace コードを他のターゲット (Whitespace, grayspace等) にコンパイルする。

**実装に必要な要素**:

1. **コンパイラフロントエンド**:
   - すでに実装済み (token_parser, tree_parser, semantic_analyzer)
   
2. **コンパイラバックエンド**:
   - IR (中間表現) の定義
   - ターゲット別のコード生成
   - 最適化パス (オプション)

3. **ターゲット**:
   - Whitespace (compiler_ws として一部実装中)
   - grayspace (未実装)
   - その他 (将来)

**参照**:
- [ai-docs/architecture/overview.md](../architecture/overview.md#5-compiler-コンパイラ---未実装)
- [ai-docs/architecture/modules.md](../architecture/modules.md#compiler)
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

**優先度**: 低 - 大規模な実装が必要

---

## 2. grayspace ターゲット

**状態**: ❌ 未実装

**説明**: grayspace ターゲットへのコンパイルは未実装。

**ディレクトリ**: `src/compiler/grayspace/` (存在するが未実装の可能性)

**目的**: nospace コードを grayspace 言語にコンパイルする。

**関連**: Whitespace コンパイラ (compiler_ws) の実装が進んでいるため、同様のアーキテクチャを使用できる可能性がある。

**参照**:
- [ai-docs/done-task/compiler-ws-implementation.md](../done-task/compiler-ws-implementation.md)

**優先度**: 低 - Whitespace コンパイラの完成後

---

## Whitespace コンパイラの状況

**状態**: ⚠️ 部分的に実装

**説明**: Whitespace へのコンパイラ機能が部分的に実装されています。

**実装済み**:
- [src/compiler_ws/](../../src/compiler_ws/) モジュール
- 基本的な型定義
- メモリレイアウト
- ラベル管理
- エンコーダ
- ビルトイン関数

**未完成**:
- 完全なコード生成
- 最適化

**参照**:
- [ai-docs/done-task/compiler-ws-implementation.md](../done-task/compiler-ws-implementation.md)
- [ai-docs/architecture/whitespace-runtime.md](../architecture/whitespace-runtime.md)

---

## 実装の優先順位

1. **Whitespace コンパイラの完成** - 部分的に実装済み
2. **grayspace コンパイラ** - Whitespace コンパイラの完成後
3. **その他のターゲット** - 将来的に検討

---

## 関連ドキュメント

- [ai-docs/architecture/overview.md](../architecture/overview.md)
- [ai-docs/architecture/modules.md](../architecture/modules.md)
- [ai-docs/done-task/compiler-ws-implementation.md](../done-task/compiler-ws-implementation.md)
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

---

## 更新履歴

- 2026-02-07: unimplemented-features.md から分離して作成
