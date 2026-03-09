# 型システム設計

nospace 言語に明示的な型注釈・構造体を導入する設計タスク。

## 概要

現在 nospace は内部的に `int` と `void` の2型を持つが、構文としての型注釈は存在しない。
本タスクでは以下を設計する:

1. **型注釈構文** (`@` 演算子): 式・変数宣言・関数パラメータ・戻り値に型を明示
2. **構造体** (`struct:`): ユーザー定義の複合型
3. **明示的キャスト** (`@ void`): 型変換
4. **型推論の維持**: 型注釈は省略可能で、既存コードとの後方互換性を保つ

## ドキュメント

- [spec.md](spec.md) - 言語仕様（構文・セマンティクス）
- [implementation.md](implementation.md) - モジュール別の実装設計
- [struct-memory-layout.md](struct-memory-layout.md) - 構造体のメモリレイアウト設計
- [dot-access-conflict.md](dot-access-conflict.md) - ドットアクセスと名前空間の関係（衝突なし: `$` 採用により解決済み）
- [nested-struct-init.md](nested-struct-init.md) - 構造体リテラル式と初期化構文（`struct: Name(...)` 構文、決定済み）

## スコープ

### 実装する

- `@` トークンの追加と型注釈構文
- `int`, `void` 型注釈
- `struct:` キーワードと構造体定義
- 構造体変数の宣言・初期化
- 構造体フィールドアクセス (`.` ドットアクセス)
- 明示的キャスト (`@ void`)
- 型チェック（注釈と推論の整合性検証）

### 実装しない（将来的に検討）

- `int[n][]` 多次元配列（型システム拡張後に検討）
- ジェネリクス
- 関数ポインタ型
- void 型変数の実体化

## 実装フェーズ

| Phase | 内容 | 依存 |
|-------|------|------|
| Phase 1 | `@` トークン追加、型注釈の構文解析 | なし |
| Phase 2 | 型注釈の意味解析（型チェック） | Phase 1 |
| Phase 3 | `struct:` 定義の構文解析・意味解析 | Phase 1 |
| Phase 4 | 構造体変数の宣言・初期化・フィールドアクセス | Phase 2, 3 |
| Phase 5 | 明示的キャスト (`@ void`) | Phase 2 |
## 既存タスクとの関連

- [namespace/](../namespace/) - 名前空間は `$` でアクセスするため `.` との衝突なし（[dot-access-conflict.md](dot-access-conflict.md) に経緯を記録）
- [temporary-array-literals.md](../temporary-array-literals.md) - 構造体初期化との相互作用
