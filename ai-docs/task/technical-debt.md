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

**状態**: ⚠️ TODO (コメント無し)

**場所**: [src/semantic_analyzer/types.rs](../../src/semantic_analyzer/types.rs#L31)

**コード**:
```rust
pub(crate) struct Variable {
    pub identifier: String,
    pub is_static: bool,
}
```

**説明**: 
- 文字列ベースの識別子管理を `IdentifierInfo` ベースに変更予定
- より効率的な識別子管理
- TODO コメントは既に削除されているが、実装は未完了

### 3.2 Function の args フィールド

**状態**: ✅ 完了 (2026-02-11)

**詳細**: [function-args-identifier-resolution.md](./function-args-identifier-resolution.md)

**説明**:
- `args: Vec<String>` フィールドは既にコードから削除済み
- コメントアウトされた行のみ残存: `// pub identifier: String,`
- `arg_indices: Vec<usize>` のみで完全に動作

### 3.3 IdentifierInfo の構造

**状態**: ⚠️ TODO (コメント無し)

**場所**: [src/semantic_analyzer/scope.rs](../../src/semantic_analyzer/scope.rs#L10-L12)

**コード**:
```rust
struct IdentifierInfo {
    // name: String,
    pub idx: usize,
}
```

**説明**:
- `name` フィールドは既にコメントアウト済み
- `idx` フィールドをより安全な型に変更予定 (newtype パターン等)
- TODO コメントは既に削除されているが、型安全性の改善は未実施

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

**状態**: ⚠️ 要確認

**説明**: コンパイル時に多数の未使用警告が出力されています。

### 5.1 未使用の関数

**現在の警告** (2026-02-11 時点):

1. `convert_to_exec_expression` (semantic_analyzer/mod.rs:220)
   - 後方互換性のため残されているが、実際には未使用
   - 全ての呼び出しは `convert_to_exec_expression_with_resolver` を使用
   - **推奨**: 削除可能

### 5.2 未使用のフィールド

**現在の警告** (2026-02-11 時点):

1. `IdentifierInfo.0` - フィールドが読み込まれていない
   - 場所: semantic_analyzer/scope.rs:10
   - 実際には `idx` という名前のフィールド
   - **原因**: 使用箇所はあるが、コンパイラが検出できていない可能性

2. `Function.scope_depth` - 書き込みのみで読み込みなし
   - 場所: semantic_analyzer/scope.rs:30
   - **推奨**: 将来の関数可視性チェックで使用予定なら保持、そうでなければ削除

3. `Scope.is_function_scope` - 未読み込み
   - 場所: semantic_analyzer/scope.rs
   - **推奨**: 関数境界チェック実装時に使用予定なら保持、そうでなければ削除

4. `IdentifierRef.is_global` - 未読み込み
   - 場所: semantic_analyzer/types.rs
   - **推奨**: グローバル変数管理で使用予定なら保持、そうでなければ削除

5. `Scope.get_variable` メソッド - 未使用

### 5.3 compiler_ws モジュールの未使用項目

**現在の警告** (2026-02-11 時点):

多数の未使用項目があります:
- `LabelId`, `WsNumber`, `HeapAddress` (未使用 import)
- `EnvironmentMetrics` (未使用 import)
- `UndefinedVariable` バリアント (未構築)
- 各種メソッド:
  - `new_label_range`, `scope`
  - `allocate_global`, `global_size`, `initial_local_heap`
  - `len`, `is_empty`, `into_instructions`, `instructions`
  - `new`, `value`, `offset`
- フィールド:
  - `global_var_count`

**原因**: compiler_ws モジュールが部分的な実装段階にあるため

**対処**:
1. 完全に不要なコードは削除
2. 将来使用予定のコードには `#[allow(dead_code)]` を追加
3. Whitespace コンパイラ実装が進んだら再評価

**優先度**: 低 - クリーンなコードベースのため

---

## 実装の優先順位

1. **未使用の関数の削除** (`convert_to_exec_expression`) - すぐに削除可能
2. **未使用フィールドの整理** - 将来使用予定か判断が必要
3. **変数/関数の識別子管理の改善** - 型安全性とパフォーマンス向上
4. ~~**Clone derive の削除**~~ - ✅ 完了 (2026-02-10)
5. ~~**Function の args フィールド削除**~~ - ✅ 完了 (2026-02-11)
6. ~~**エラーメッセージ型の改善**~~ - ✅ 完了済み

---

## 関連ドキュメント

- [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)
- [src/compiler_ws/](../../src/compiler_ws/)
- [ai-docs/spec/implementation-status.md](../spec/implementation-status.md)

---

## 更新履歴

- 2026-02-11: 実装状況を再確認し、最新の警告情報に更新。Function の args フィールド削除を完了としてマーク
- 2026-02-10: Clone derive の削除を完了、エラーメッセージ型が既に完了済みであることを確認
- 2026-02-07: unimplemented-features.md から分離して作成
