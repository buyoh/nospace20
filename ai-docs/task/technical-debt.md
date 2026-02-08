# コード内の技術的負債 (残件)

このドキュメントは nospace プロジェクトのコード内に残る技術的負債と改善点をまとめたものです。

最終更新日: 2026-02-07

## 目次

1. [Expression::Invalid の処理](#1-expressioninvalid-の処理)
2. [Clone derive の削除 (最適化)](#2-clone-derive-の削除-最適化)
3. [変数/関数の識別子管理の改善](#3-変数関数の識別子管理の改善)
4. [エラーメッセージ型の改善](#4-エラーメッセージ型の改善)
5. [未使用の関数とフィールド](#5-未使用の関数とフィールド)

---

## 1. Expression::Invalid の処理

**状態**: ✅ 実装済み (unreachable!)

**場所**: [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs#163)

**コード**:
```rust
Expression::Invalid(_) => {
    unreachable!("Expression::Invalid should not reach semantic analysis")
}
```

**説明**: 
- パース時にエラーとなった Invalid な式の処理は `unreachable!()` で実装済み
- パースエラー時のみ Invalid が生成されるため、正常系では到達しない
- 以前は `todo!()` だったが、現在は適切に処理されている

**優先度**: なし (完了)

---

## 2. Clone derive の削除 (最適化)

**状態**: ⚠️ TODO

**場所**: [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)

**コード**:
```rust
// #[derive(Clone)] // TODO: REMOVE
pub(crate) enum ExecExpression { ... }

// #[derive(Clone)] // TODO: REMOVE
pub(crate) enum ExecStatement { ... }
```

**説明**: 
- パフォーマンス最適化のため、不要な `Clone` derive を削除予定
- 現在はコメントアウトされているが、完全に削除すべきか検証が必要

**影響**:
- メモリ使用量の削減
- コンパイル時間の短縮

**優先度**: 低 - パフォーマンス最適化

---

## 3. 変数/関数の識別子管理の改善

**状態**: ⚠️ TODO

**場所**: [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)

### 3.1 Variable の identifier フィールド

**コード**:
```rust
pub(crate) struct Variable {
    pub identifier: String, // TODO: use IdentifierInfo
    pub is_static: bool,
}
```

**説明**: 
- 文字列ベースの識別子管理を `IdentifierInfo` ベースに変更予定
- より効率的な識別子管理

### 3.2 Function の args フィールド

**コード**:
```rust
pub(crate) struct Function {
    pub args: Vec<String>, // TODO: change string to identifier_ptr
    pub arg_indices: Vec<usize>,
    pub block: Block,
}
```

**説明**:
- 引数名を文字列から識別子ポインタに変更予定
- より型安全な実装

### 3.3 IdentifierInfo の idx フィールド

**コード**:
```rust
struct IdentifierInfo {
    idx: usize, // TODO: more safety
}
```

**説明**:
- より安全な型に変更予定 (newtype パターン等)

**優先度**: 中 - 型安全性とパフォーマンス向上

---

## 4. エラーメッセージ型の改善

**状態**: ⚠️ 設計中

**説明**: `CodeParseError.message` を `Cow<'static, str>` に変更することを検討中。

**詳細**: [error-message-improvement.md](./error-message-improvement.md) を参照

**優先度**: 低 - パフォーマンス最適化

---

## 5. 未使用の関数とフィールド

**状態**: ⚠️ 要確認

**説明**: コンパイル時に多数の未使用警告が出力されています。

### 5.1 未使用の関数

- `convert_to_exec_expression` (semantic_analyzer/mod.rs:169)
  - コメントで「削除予定」と記載あり
  - 全ての呼び出しを `convert_to_exec_expression_with_resolver` に置き換える

### 5.2 未使用のフィールド

- `IdentifierInfo.0` (semantic_analyzer/mod.rs:45)
- `is_function_scope` (semantic_analyzer/mod.rs:216)
- `is_global` (compiler_ws/context.rs:34)
- その他多数

### 5.3 compiler_ws モジュールの未使用項目

多数の未使用項目があります:
- `LabelId`, `WsNumber`, `HeapAddress` (未使用 import)
- `UndefinedVariable` (未構築バリアント)
- 各種メソッド (`len`, `is_empty`, `allocate_global`, 等)

**原因**: compiler_ws モジュールが部分的な実装段階にあるため

**対処**:
1. 完全に不要なコードは削除
2. 将来使用予定のコードには `#[allow(dead_code)]` を追加
3. コンパイラ実装が進んだら再評価

**優先度**: 低 - クリーンなコードベースのため

---

## 実装の優先順位

1. **変数/関数の識別子管理の改善** - 型安全性とパフォーマンス向上
2. **Clone derive の削除** - パフォーマンス最適化
3. **未使用の関数とフィールドの整理** - コードの可読性向上
4. **エラーメッセージ型の改善** - パフォーマンス最適化

---

## 関連ドキュメント

- [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)
- [src/compiler_ws/](../../src/compiler_ws/)
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

---

## 更新履歴

- 2026-02-07: unimplemented-features.md から分離して作成
