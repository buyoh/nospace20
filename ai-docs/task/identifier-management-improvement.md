# 識別子管理の改善設計

technical-debt.md §3.1, §3.3 を分離したドキュメント。

## 概要

semantic_analyzer における識別子管理の技術的負債を解消する。

1. `Variable.identifier: String` フィールドの削除
2. `IdentifierInfo` 構造体の型安全化

## 1. Variable.identifier フィールドの削除

### 現状

```rust
// src/semantic_analyzer/types.rs
pub(crate) struct Variable {
    pub identifier: String,
    pub is_static: bool,
    pub array_size: Option<usize>,
}
```

`Variable.identifier` は以下の目的で使用されている:

| # | ファイル | 行 | 使用目的 |
|---|---------|-----|----------|
| 1 | scope.rs | L304 | `ScopeBuilder.build()` で `variable_indices` (名前→スロットindex) 構築 |
| 2 | scope.rs | L305 | `ScopeBuilder.build()` で `variable_name_to_var_index` (名前→Variable配列index) 構築 |
| 3 | mod.rs | L335 | `analyze_internal_with_parent` で temporary_scope 用の同等 map 構築 |
| 4 | mod.rs | L336 | 同上 |
| 5 | exec.rs | L220 | `interpret_call_user_function` で static 変数の slot_idx を `variable_indices[&var.identifier]` で取得 |
| 6 | exec.rs | L289 | `interpret_call_user_function_by_ref` で同様 |

### 設計方針

#### A. Variable にスロットインデックスを直接保持する

`Variable` に `slot_index: usize` フィールドを追加し、`identifier` を削除する。

```rust
pub(crate) struct Variable {
    pub slot_index: usize,
    pub is_static: bool,
    pub array_size: Option<usize>,
}
```

**影響**:
- `ScopeBuilder.build()` (使用箇所 1, 2): `variable_indices` / `variable_name_to_var_index` map は `ScopeBuilder` が持つ `identifier_map` と `Variable.slot_index` から構築可能。ただし名前→index map は `ScopeBuilder` 側で変数追加時に構築するのが自然。
- `mod.rs:335-336` (使用箇所 3, 4): 同様に `slot_index` から直接構築。
- `exec.rs:220, 289` (使用箇所 5, 6): `variable_indices[&var.identifier]` を `var.slot_index` に置換。最も大きな改善点。

**利点**:
- interpreter での名前ベースの `BTreeMap` lookup が不要になる
- `Variable` の `Clone` コストが低減 (String 不要)
- 一貫した数値ベースの識別子管理

**欠点**:
- `slot_index` を `Variable` 構築時に確定させる必要がある → `ScopeBuilder.add_variable` で計算可能

#### B. ScopeBuilder の変数 map 構築を改善する（Variable から identifier を完全に削除）

`ScopeBuilder.add_variable` 呼び出し時に `name` を受け取り、内部で `variable_indices` / `variable_name_to_var_index` を即座に構築する。`Variable` 構造体から `identifier` フィールドを完全に削除する。

`ScopeBuilder` の中で変数追加時に:
1. `identifier_map` に `name → Identifier::Variable(...)` を追加（既存）
2. 内部の `variable_indices_builder` に `name → slot_index` を追加（新規）
3. 内部の `variable_name_to_var_index_builder` に `name → var_index` を追加（新規）

`build()` 時にはこれらをそのまま `Scope` に渡す。

**利点**:
- `Variable` が完全に名前非依存になる
- `build()` 内のループが不要

**欠点**:
- `ScopeBuilder` のフィールドが増える
- `mod.rs` の temporary_scope 構築ロジックの修正が必要

### 推奨: 方針 A

方針 A が変更量が少なく、段階的に適用しやすい。

### 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/semantic_analyzer/types.rs` | `Variable` から `identifier` を削除、`slot_index` を追加 |
| `src/semantic_analyzer/scope.rs` | `ScopeBuilder.add_variable` で `slot_index` 計算、`build()` 簡略化 |
| `src/semantic_analyzer/mod.rs` | `Variable` 構築時に `slot_index` を渡す。temporary_scope 構築修正 |
| `src/interpreter/exec.rs` | `variable_indices[&var.identifier]` → `var.slot_index` |

### 変更量

小（4ファイル、各ファイル数行）

---

## 2. IdentifierInfo 構造体の型安全化

### 現状

```rust
// src/semantic_analyzer/scope.rs
#[derive(Clone)]
pub(super) struct IdentifierInfo {
    // name: String,
    pub idx: usize,
}

#[derive(Clone)]
pub(super) enum Identifier {
    Function(IdentifierInfo),
    Variable(IdentifierInfo),
}
```

`IdentifierInfo` は `idx: usize` のみのラッパーで、関数・変数で同じ型を使い回している。型安全性がない。

### 設計方針

#### C. newtype パターンで分離

関数インデックスと変数インデックスを別の型にする。

```rust
#[derive(Clone, Copy)]
pub(super) struct FunctionIndex(pub usize);

#[derive(Clone, Copy)]
pub(super) struct VariableIndex(pub usize);

#[derive(Clone)]
pub(super) enum Identifier {
    Function(FunctionIndex),
    Variable(VariableIndex),
}
```

**利点**:
- 関数インデックスと変数インデックスの混同を型レベルで防止
- `IdentifierInfo` を抹消でき、コメントアウトされた `name` も整理
- `Copy` derive で利便性向上

**欠点**:
- 使用箇所全てで型を合わせる必要があるが、`pub(super)` なので影響範囲は semantic_analyzer 内のみ

#### D. IdentifierInfo のまま Copy を追加

```rust
#[derive(Clone, Copy)]
pub(super) struct IdentifierInfo {
    pub idx: usize,
}
```

最小限の変更でコメント行の削除と `Copy` 追加のみ行う。

### 推奨: 方針 C

`pub(super)` なので影響範囲は semantic_analyzer 内のみ。型安全性の向上効果が高い。

### 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/semantic_analyzer/scope.rs` | `IdentifierInfo` → `FunctionIndex` / `VariableIndex` に分離 |
| `src/semantic_analyzer/mod.rs` | `IdentifierInfo { idx: ... }` → `FunctionIndex(...)` に変更 |

### 変更量

小（2ファイル、各ファイル数行）

---

## 実装順序

1. **Phase 1**: IdentifierInfo の型安全化 (§2, 方針 C)
   - 影響範囲が小さく独立して実施可能
2. **Phase 2**: Variable.identifier の削除 (§1, 方針 A)
   - Phase 1 と独立

各 Phase は独立しており、どちらを先に実施しても問題ない。

## 関連ドキュメント

- [technical-debt.md](technical-debt.md) - 元の技術的負債ドキュメント
- [src/semantic_analyzer/types.rs](../../src/semantic_analyzer/types.rs)
- [src/semantic_analyzer/scope.rs](../../src/semantic_analyzer/scope.rs)
- [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)
- [src/interpreter/exec.rs](../../src/interpreter/exec.rs)
