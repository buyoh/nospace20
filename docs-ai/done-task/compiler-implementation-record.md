# コンパイラ実装状況記録

このドキュメントは nospace プログラミング言語のコンパイラ実装状況を記録したものです。

作成日: 2026-02-07  
最終更新日: 2026-02-18  
ステータス: ✅ Whitespace コンパイラほぼ完成

## 目次

1. [Whitespace コンパイラ実装完了](#1-whitespace-コンパイラ実装完了)
2. [grayspace ターゲット](#2-grayspace-ターゲット)
3. [今後の展望](#3-今後の展望)

---

## 1. Whitespace コンパイラ実装完了

**状態**: ✅ ほぼ完成 (98.9%テスト成功率)

**実装完了日**: 2026-02-16 (主要機能)、2026-02-18 (static変数)

**説明**: nospace から Whitespace へのコンパイラが `src/compiler_ws/` モジュールとして実装され、ほぼ完成しています。

**実装済み機能**:
- ✅ 全ての算術・比較・論理演算
- ✅ 変数（グローバル・ローカル・static）
- ✅ 配列とポインタ
- ✅ ユーザー定義関数の定義と呼び出し
- ✅ if/while 式、break/continue
- ✅ return 文
- ✅ ブロックスコープ
- ✅ 標準入出力 (`__puti`, `__putc`, `__geti`, `__getc`)
- ✅ デバッグビルトイン

**テスト結果** (2026-02-18):
```
287 passed; 3 failed; 124 ignored
成功率: 98.9%
```

**既知の問題**:
- ⚠️ ラベル重複バグ (3件) - [compiler-ws-duplicate-label-bug.md](../task/compiler-ws-duplicate-label-bug.md) で管理中
  - 関数スコープのシャドーイングでラベルIDが重複
  - 影響: `func_shadowing_*` テスト3件のみ

**参照**:
- [compiler-ws-implementation.md](compiler-ws-implementation.md) - 初期実装
- [compiler-whitespace-missing-features-implementation.md](compiler-whitespace-missing-features-implementation.md) - 追加機能実装
- [whitespace-static-variable-issue.md](whitespace-static-variable-issue.md) - static変数実装
- [../architecture/whitespace-runtime.md](../architecture/whitespace-runtime.md) - アーキテクチャ

---

## 2. grayspace ターゲット

**状態**: ❌ 未実装

**説明**: grayspace ターゲットへのコンパイルは未実装。

**ディレクトリ**: `src/compiler/grayspace/` (存在するが未実装の可能性)

**目的**: nospace コードを grayspace 言語にコンパイルする。

**関連**: Whitespace コンパイラ (compiler_ws) の実装が進んでいるため、同様のアーキテクチャを使用できる可能性がある。

**参照**:
- [compiler-ws-implementation.md](compiler-ws-implementation.md)

**優先度**: 低 - Whitespace コンパイラの完成後

---



## 3. 今後の展望

### 短期タスク

1. **Whitespace コンパイラのバグ修正** - 進行中
   - ラベル重複バグの修正 ([compiler-ws-duplicate-label-bug.md](../task/compiler-ws-duplicate-label-bug.md))

### 長期的な計画

1. **grayspace コンパイラ** - 未着手
   - Whitespace コンパイラの実装をベースに展開可能
2. **その他のターゲット** - 検討段階
   - LLVM IR、WebAssembly など

---

## 関連ドキュメント

- [../architecture/overview.md](../architecture/overview.md)
- [../architecture/modules.md](../architecture/modules.md)
- [compiler-ws-implementation.md](compiler-ws-implementation.md)
- [../spec/implementation-status.md](../spec/implementation-status.md)

---

## 更新履歴

- 2026-02-18: Whitespace コンパイラの実装完了を反映、ドキュメント全体を更新
- 2026-02-07: unimplemented-features.md から分離して作成
