# コード内の技術的負債 (残件)

このドキュメントは nospace プロジェクトのコード内に残る技術的負債と改善点をまとめたものです。

最終更新日: 2026-02-11

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

**状態**: ✅ 完了 (2026-02-10)

**場所**: [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)

**説明**: 
- ExecExpression と ExecStatement から Clone derive のコメント行を削除
- コードベース全体で clone() 呼び出しが存在しないことを確認済み
- 既存のテストがすべてパスすることを確認

**影響**:
- コードの可読性向上
- 不要なコメントの削除

**優先度**: なし (完了)

---

## 3. 変数/関数の識別子管理の改善

**状態**: ⚠️ TODO

**場所**: [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)

### 3.1 Variable の identifier フィールド

**状態**: ⚠️ TODO → 別ドキュメントに分離

**詳細**: [identifier-management-improvement.md](./identifier-management-improvement.md) §1

**概要**: `Variable.identifier: String` を削除し、`slot_index: usize` に置き換える。interpreter での名前ベース lookup が不要になる。変更量: 小（4ファイル）。

### 3.2 Function の args フィールド

**状態**: ✅ 完了 (2026-02-11)

**説明**:
- `args: Vec<String>` フィールドは既にコードから削除済み
- `arg_indices: Vec<usize>` のみで完全に動作

### 3.3 IdentifierInfo の型安全化

**状態**: ⚠️ TODO → 別ドキュメントに分離

**詳細**: [identifier-management-improvement.md](./identifier-management-improvement.md) §2

**概要**: `IdentifierInfo` を `FunctionIndex` / `VariableIndex` の newtype に分離。関数・変数インデックスの混同を型レベルで防止。変更量: 小（2ファイル）。

**優先度**: 中 - 型安全性とパフォーマンス向上

---

## 4. エラーメッセージ型の改善

**状態**: ✅ 完了済み

**説明**: `CodeParseError.message` は既に `Cow<'static, str>` を使用しています。

**場所**: [src/base/mod.rs](../../src/base/mod.rs#L5)

**実装**:
```rust
pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: Cow<'static, str>,
    // ...
}
```

この改善は既に実装されており、追加の作業は不要です。

**優先度**: なし (完了済み)

---

## 5. 未使用の関数とフィールド

**状態**: ⚠️ TODO → 別ドキュメントに分離

**詳細**: [unused-code-cleanup.md](./unused-code-cleanup.md)

**概要**: `cargo build` で出力される 17 件の未使用警告を調査・分類済み。
- semantic_analyzer: 完全デッドコード 4 件 → 削除
- compiler_ws: 部分実装による未使用 → `#[allow(dead_code)]` / 不要 import 除去
- interpreter: `EnvironmentMetrics` re-export → `#[allow(dead_code)]`

**優先度**: 低～中

---

## 実装の優先順位

全ての項目が完了または別ドキュメントに分離済み。

1. ~~**未使用の関数の削除**~~ → [unused-code-cleanup.md](./unused-code-cleanup.md)
2. ~~**未使用フィールドの整理**~~ → [unused-code-cleanup.md](./unused-code-cleanup.md)
3. ~~**変数/関数の識別子管理の改善**~~ → [identifier-management-improvement.md](./identifier-management-improvement.md)
4. ~~**Clone derive の削除**~~ - ✅ 完了 (2026-02-10)
5. ~~**Function の args フィールド削除**~~ - ✅ 完了 (2026-02-11)
6. ~~**エラーメッセージ型の改善**~~ - ✅ 完了済み

---

## 関連ドキュメント

- [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)
- [src/compiler_ws/](../../src/compiler_ws/)
- [docs-ai/spec/implementation-status.md](../spec/implementation-status.md)
- [identifier-management-improvement.md](./identifier-management-improvement.md) - §3.1, §3.3 の詳細設計
- [unused-code-cleanup.md](./unused-code-cleanup.md) - §5 の詳細調査・対処方針

---

## 更新履歴

- 2026-02-11: §5 を unused-code-cleanup.md に分離。本ドキュメントを完了として done-task/ に移動
- 2026-02-11: §3.1, §3.3 の設計を identifier-management-improvement.md に分離
- 2026-02-11: 実装状況を再確認し、最新の警告情報に更新。Function の args フィールド削除を完了としてマーク
- 2026-02-10: Clone derive の削除を完了、エラーメッセージ型が既に完了済みであることを確認
- 2026-02-07: unimplemented-features.md から分離して作成
