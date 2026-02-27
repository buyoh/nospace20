# 未実装の変数関連機能

このドキュメントは nospace プログラミング言語における未実装の変数関連機能をまとめたものです。

最終更新日: 2026-02-10

## 目次

1. [final / const 変数](#1-final--const-変数)

---

## 1. final / const 変数

**状態**: ❌ 未実装

**説明**: 再代入不可の変数を定義する機能は未実装。

### 1.1 final 変数

**説明**: 一度だけ代入可能で、その後は再代入不可の変数。

**構文例**:
```nospace
func: main() {
  final:x;   # 再代入不可 #
  x = 10;    # OK: 初回の代入 #
  # x = 20;  # エラー: 再代入不可 #
}
```

### 1.2 const 変数

**説明**: リテラルのみ代入可能かつ再代入不可の変数（定数）。

**構文例**:
```nospace
func: main() {
  const:PI = 3;  # リテラルのみ代入可かつ再代入不可 #
}
```

**実装に必要な変更**:

1. **トークンパーサ**:
   - `final:` と `const:` キーワードを認識
   
2. **構文解析器**:
   - `VariableDeclaration` に mutability フラグを追加
   
3. **意味解析器**:
   - `Variable` 構造体に mutability 情報を追加
   - 再代入の検証ロジックを追加
   
4. **エラーハンドリング**:
   - 再代入エラーの検出と報告

**参照**:
- [docs/spec.md](../../docs/spec.md) セクション 4
- テスト: [disabled_var_final_001.ns](../../resources/tests/passes/variables/disabled_var_final_001.ns)

**優先度**: 中 - 安全性向上

---

## 実装の優先順位

1. **final / const 変数** - 安全性とコードの意図を明確にする

---

## 関連ドキュメント

- [docs/spec.md](../../docs/spec.md) - 言語仕様
- [ai-docs/done-task/block-scope-global-variables-implementation.md](../done-task/block-scope-global-variables-implementation.md) - 実装済みの変数機能
- [ai-docs/done-task/implement-multi-variable-declaration.md](../done-task/implement-multi-variable-declaration.md) - 実装済みの変数初期化機能
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md) - 実装状況の詳細

---

## 更新履歴

- 2026-02-10: 変数初期化機能が実装済みのため、該当セクションを削除
- 2026-02-07: unimplemented-features.md から分離して作成
