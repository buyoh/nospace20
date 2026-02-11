# 未使用コードの整理

technical-debt.md §5 を分離したドキュメント。

## 概要

`cargo build` で出力される 17 件の未使用警告を整理する。
調査日: 2026-02-11

## 分類方針

| 分類 | 対処 |
|------|------|
| 完全デッドコード | 削除 |
| 将来使用予定（compiler_ws 部分実装） | `#[allow(dead_code)]` |
| テスト専用 | `#[cfg(test)]` 化 |
| 不要 import | 除去 |

---

## 1. semantic_analyzer モジュール（削除推奨）

### 1.1 `convert_to_exec_expression` 関数

- **場所**: [src/semantic_analyzer/mod.rs:220](../../src/semantic_analyzer/mod.rs#L220)
- **状態**: 呼び出し元なし。全箇所が `convert_to_exec_expression_with_resolver` を直接使用
- **対処**: **削除**

### 1.2 `Function.scope_depth` フィールド

- **場所**: [src/semantic_analyzer/scope.rs:30](../../src/semantic_analyzer/scope.rs#L30)
- **状態**: 書き込みのみ（[mod.rs:294](../../src/semantic_analyzer/mod.rs#L294), [mod.rs:450](../../src/semantic_analyzer/mod.rs#L450)）、読み込みなし
- **注意**: `IdentifierRef.scope_depth` とは別のフィールド（そちらは interpreter で使用中）
- **対処**: **削除**（書き込み箇所も合わせて削除）

### 1.3 `Scope.is_function_scope` フィールド

- **場所**: [src/semantic_analyzer/scope.rs:71](../../src/semantic_analyzer/scope.rs#L71)
- **状態**: `ScopeBuilder::build()` で設定されるが、出力後の `Scope` からは読まれない
- **注意**: `ScopeInfo.is_function_scope` は `ScopeResolver` 内部で使用中（別構造体）
- **対処**: **削除**（`ScopeBuilder::build()` の引数 `is_function_scope` も不要に）

### 1.4 `Scope::get_variable` メソッド

- **場所**: [src/semantic_analyzer/scope.rs:92](../../src/semantic_analyzer/scope.rs#L92)
- **状態**: 呼び出し元なし
- **対処**: **削除**（`get_function` と対称だが、必要になった時に再追加可能）

### 1.5 `Identifier::Variable(IdentifierInfo)` の内部値が未読

- **場所**: [src/semantic_analyzer/scope.rs:18](../../src/semantic_analyzer/scope.rs#L18)
- **状態**: `get_variable` が `info.idx` を読んでいたが、`get_variable` 自体が未使用
- **対処**: `get_variable` 削除後も `Identifier::Variable` variant 自体は `identifier_map` 登録で必要。`idx` が完全未読になるが、identifier-management-improvement.md で対処予定

---

## 2. compiler_ws モジュール（部分実装のため `#[allow(dead_code)]` 推奨）

Whitespace コンパイラは実装途中。基本機能は動作しており、テストも通っている。
将来の実装拡張で使用される設計済みコードが多い。

### 2.1 不要 import（即座に修正可能）

| ファイル | import | 対処 |
|---------|--------|------|
| [builtin.rs:9](../../src/compiler_ws/builtin.rs#L9) | `LabelId` | **import から除去** |
| [mod.rs:30](../../src/compiler_ws/mod.rs#L30) | `HeapAddress`, `LabelId`, `WsNumber` | **import から除去** |

### 2.2 未使用トレイト

| ファイル | 項目 | 対処 |
|---------|------|------|
| [encoder.rs:6](../../src/compiler_ws/encoder.rs#L6) | `WsEncode` トレイト | **削除**（定義のみで実装なし。必要になったら再作成） |

### 2.3 未構築バリアント

| ファイル | 項目 | 対処 |
|---------|------|------|
| [mod.rs:38](../../src/compiler_ws/mod.rs#L38) | `UndefinedVariable` バリアント | `#[allow(dead_code)]` — エラーハンドリング拡張で使用予定 |

### 2.4 フィールド・メソッド

| ファイル | 項目 | 対処 |
|---------|------|------|
| [context.rs:31](../../src/compiler_ws/context.rs#L31) | `is_global` フィールド | `#[allow(dead_code)]` |
| [context.rs:75](../../src/compiler_ws/context.rs#L75) | `new_label_range`, `scope` メソッド | `#[allow(dead_code)]` |
| [label.rs:79](../../src/compiler_ws/label.rs#L79) | `has_function` メソッド | `#[cfg(test)]` 化（テストでのみ使用） |
| [memory.rs:10](../../src/compiler_ws/memory.rs#L10) | `global_var_count` フィールド | `#[allow(dead_code)]` |
| [memory.rs:38](../../src/compiler_ws/memory.rs#L38) | `allocate_global`, `global_size`, `initial_local_heap` | `#[allow(dead_code)]` |
| [program.rs:57](../../src/compiler_ws/program.rs#L57) | `len`, `is_empty`, `into_instructions`, `instructions` | `#[allow(dead_code)]` |
| [types.rs:84](../../src/compiler_ws/types.rs#L84) | `HeapAddress::new`, `value`, `offset` | `#[allow(dead_code)]` |

---

## 3. interpreter モジュール

### 3.1 `EnvironmentMetrics` の未使用 re-export

- **場所**: [src/interpreter/mod.rs:12](../../src/interpreter/mod.rs#L12)
- **状態**: `pub use` で re-export されているが、`lib.rs` からは re-export されていない。`metrics()` メソッドも外部から呼ばれていない
- **対処**: `#[allow(dead_code)]` — 将来のデバッグ・プロファイリング用途で有用

---

## 実装順序

### Phase 1: 即座に削除可能なデッドコード

1. `convert_to_exec_expression` 関数 (§1.1)
2. `Function.scope_depth` フィールドと書き込み箇所 (§1.2)
3. `Scope.is_function_scope` フィールドと `build()` 引数 (§1.3)
4. `Scope::get_variable` メソッド (§1.4)

### Phase 2: compiler_ws の不要 import・トレイト削除

1. `LabelId` 等の不要 import (§2.1)
2. `WsEncode` トレイト (§2.2)

### Phase 3: `#[allow(dead_code)]` の追加

1. compiler_ws の将来使用予定コード (§2.3, §2.4)
2. interpreter の `EnvironmentMetrics` (§3.1)

---

## 関連ドキュメント

- [technical-debt.md](../done-task/technical-debt.md) - 元の技術的負債ドキュメント（完了済み）
- [identifier-management-improvement.md](identifier-management-improvement.md) - 識別子管理改善（§1.5 と関連）
- [src/semantic_analyzer/](../../src/semantic_analyzer/) - semantic_analyzer モジュール
- [src/compiler_ws/](../../src/compiler_ws/) - compiler_ws モジュール
- [src/interpreter/](../../src/interpreter/) - interpreter モジュール
