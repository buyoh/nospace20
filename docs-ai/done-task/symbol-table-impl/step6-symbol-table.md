# ステップ6: SymbolTable の導入

親ドキュメント: [README.md](README.md)  
作成日: 2026-02-17  
前提: ステップ5 完了

## 概要

デバッグ用 `SymbolTable` 構造体を導入し、文字列ベースの識別子情報を分離する。
ランタイムコードはインデックスベースで動作し、文字列情報は `SymbolTable` を通じてのみアクセスする。

## 目的

1. **責務の明確化**: ランタイム（interpreter / compiler_ws）が文字列情報に直接依存しない設計
2. **将来の最適化**: `--no-debug-symbols` オプションで SymbolTable 省略可能にする準備
3. **Scope 構造体の簡素化**: 文字列マッピングを一箇所に集約

## 現在の文字列フィールドの使用状況

### Scope 上の文字列フィールド

| フィールド | 型 | 使用箇所 | ランタイム使用 |
|-----------|---|---------|-------------|
| `identifier_map` | `BTreeMap<String, Identifier>` | semantic_analyzer 内部、`get_function()`, `has_function()` | ⚠️ 公開 API のみ |
| `variable_indices` | `BTreeMap<String, usize>` | semantic_analyzer 内部（ScopeResolver） | ❌ なし |
| `variable_name_to_var_index` | `BTreeMap<String, usize>` | semantic_analyzer 内部（ScopeResolver） | ❌ なし |
| `function_names` | `Vec<String>` | compiler_ws（ラベル生成）、interpreter/mod.rs（ステップ5後は不使用） | ⚠️ compiler_ws のみ |

### 調査結果

- **`variable_indices`** / **`variable_name_to_var_index`**: interpreter / compiler_ws では一切使用されていない。semantic_analyzer の `ScopeResolver` が使用するのみ。ただし `Scope` のフィールドとして各ブロックに残存している。
- **`identifier_map`**: interpreter / compiler_ws では直接使用されていない。`Scope::get_function(name)` と `Scope::has_function(name)` の公開メソッドを通じてのみ使用される。
- **`function_names`**: interpreter/mod.rs では `initialize_function_statics` でキー生成に使用（ステップ5 で不要に）。compiler_ws では関数ラベル生成に使用。

## 設計

### SymbolTable 構造体

```rust
/// デバッグ用シンボルテーブル
///
/// インデックスから識別子名への逆引きを提供する。
/// ランタイム動作には不要だが、デバッグ出力・エラーメッセージ・
/// コンパイラのラベル生成で使用される。
pub struct SymbolTable {
    /// 関数インデックス → 関数名
    pub function_names: Vec<String>,
    /// 関数名 → 関数インデックス（逆引き）
    pub function_name_to_index: BTreeMap<String, usize>,
}
```

### Scope の変更

```rust
pub struct Scope {
    // --- ランタイム用（インデックスベース） ---
    pub(crate) variables: Vec<Variable>,
    pub(crate) variable_count: usize,
    pub(crate) functions: Vec<Function>,
    pub main_function_index: Option<usize>,
    pub(crate) static_init_statements: Vec<ExecStatement>,
    pub(crate) root_statements: Vec<ExecStatement>,

    // --- シンボルテーブル（文字列情報） ---
    /// デバッグ用シンボルテーブル
    pub symbol_table: SymbolTable,

    // --- semantic_analyzer 内部用（ランタイムでは未使用） ---
    pub(super) identifier_map: BTreeMap<String, Identifier>,
    pub(crate) variable_indices: BTreeMap<String, usize>,
    pub(crate) variable_name_to_var_index: BTreeMap<String, usize>,
}
```

**設計判断**: `symbol_table` は常に存在する（`Option<SymbolTable>` ではなく `SymbolTable`）。
理由:
- compiler_ws が関数名を必要とするため、コンパイル時には必須
- `has_function(name)` などの公開 API が名前解決を必要とする
- 将来的に省略可能にする場合は `Option<SymbolTable>` に変更するだけで済む

### 公開 API の変更

```rust
impl Scope {
    /// 関数名から関数インデックスを解決する（SymbolTable を使用）
    pub fn resolve_function_index(&self, name: &str) -> Option<usize> {
        self.symbol_table.function_name_to_index.get(name).copied()
    }

    /// 指定した名前の関数が存在するかチェックする
    pub fn has_function(&self, name: &str) -> bool {
        self.symbol_table.function_name_to_index.contains_key(name)
    }

    /// 関数名から関数への参照を取得（従来の get_function の置き換え）
    pub fn get_function(&self, name: &str) -> Option<&Function> {
        let idx = self.resolve_function_index(name)?;
        Some(&self.functions[idx])
    }

    /// 関数インデックスから関数名を取得
    pub fn get_function_name(&self, index: usize) -> Option<&str> {
        self.symbol_table.function_names.get(index).map(|s| s.as_str())
    }
}
```

## 実装サブステップ

### 6a: SymbolTable 構造体の定義

**対象ファイル**: `src/semantic_analyzer/scope.rs`（新しい型を追加）

1. `SymbolTable` 構造体を定義
2. `function_names: Vec<String>` フィールドを持つ
3. `function_name_to_index: BTreeMap<String, usize>` フィールドを持つ

### 6b: Scope に SymbolTable を統合

**対象ファイル**: `src/semantic_analyzer/scope.rs`, `src/semantic_analyzer/mod.rs`

1. `Scope.function_names` を `Scope.symbol_table.function_names` に移動
2. `ScopeBuilder.build()` で `SymbolTable` を構築
3. `function_name_to_index` を `function_names` から構築
4. `get_function()` / `has_function()` を `symbol_table` 経由に変更（`identifier_map` を使わない）

### 6c: compiler_ws の更新

**対象ファイル**: `src/compiler_ws/statement.rs`, `src/compiler_ws/expression.rs`

1. `scope.function_names` → `scope.symbol_table.function_names` に変更
2. `scope.function_names[func_ref.local_index]` → `scope.symbol_table.function_names[func_ref.local_index]` に変更

### 6d: identifier_map の縮小

**対象ファイル**: `src/semantic_analyzer/scope.rs`, `src/semantic_analyzer/mod.rs`

`identifier_map` の `Identifier::Function` エントリは `SymbolTable.function_name_to_index` に代替される。
`get_function()` は `identifier_map` ではなく `symbol_table` を使用するため、
`identifier_map` に `Function` エントリを保持する必要がなくなる。

ただし、`identifier_map` は `ScopeResolver` が関数解決に使用しているため、
semantic_analyzer 内部では引き続き必要。最終的な Scope 出力からの削除は、
`variable_indices` / `variable_name_to_var_index` の整理と合わせて検討する。

### 6e: 変数名情報の SymbolTable 統合（将来拡張）

将来的に変数名のデバッグ情報も SymbolTable に移動する場合:

```rust
pub struct SymbolTable {
    pub function_names: Vec<String>,
    pub function_name_to_index: BTreeMap<String, usize>,
    /// 変数スロットインデックス → 変数名（将来拡張）
    pub variable_names: Vec<Option<String>>,
}
```

この拡張はステップ6の必須要件ではなく、エラーメッセージ改善時に検討する。

## 各ファイルの変更一覧

### `src/semantic_analyzer/scope.rs`
- `SymbolTable` 構造体を新規定義
- `Scope` に `symbol_table: SymbolTable` フィールド追加
- `Scope.function_names` を削除（`symbol_table.function_names` に移行）
- `get_function()` を `symbol_table.function_name_to_index` ベースに変更
- `has_function()` を同様に変更
- `ScopeBuilder.build()` で `SymbolTable` を構築

### `src/semantic_analyzer/mod.rs`
- `global_function_names` の扱いは変更なし（ビルド時に SymbolTable に渡される）

### `src/compiler_ws/statement.rs`
- `scope.function_names` → `scope.symbol_table.function_names`

### `src/compiler_ws/expression.rs`
- `scope.function_names[...]` → `scope.symbol_table.function_names[...]`

### `src/interpreter/mod.rs`
- `scope.function_names` 参照箇所は、ステップ5 完了後には存在しない（変更不要）

### `src/bin/nospace20.rs`
- `a.has_function("main")` — API は変わらないため変更不要

### `src/lib.rs`
- `interpret_func` 系公開 API — 内部で `scope.get_function(func_name)` を使用しており、
  API シグネチャに変更なし

### テストコード
- `src/semantic_analyzer/tests.rs`: `scope.get_function("main")` — API は同一のため変更不要

## テスト方針

- 既存テスト（unit テスト・large テスト）がそのまま回帰テストとして機能
- `get_function`、`has_function` の動作が変わらないことを確認
- compiler_ws のテスト（whitespace コンパイル→実行）が PASS することを確認

## 完了条件

- [ ] `SymbolTable` 構造体が `src/semantic_analyzer/scope.rs` に定義されている
- [ ] `Scope.function_names` が `Scope.symbol_table.function_names` に移動されている
- [ ] `Scope.get_function()` / `has_function()` が `symbol_table` 経由で動作している
- [ ] compiler_ws が `symbol_table.function_names` を使用している
- [ ] interpreter が `symbol_table` に一切依存していない（インデックスベースのみ）
- [ ] `cargo test` が全て PASS
- [ ] 全公開 API のシグネチャに変更がない

## 将来の展望

ステップ6 完了後の状態:

| 項目 | 状態 |
|-----|------|
| ランタイム（interpreter） | 完全にインデックスベース |
| コンパイラ（compiler_ws） | SymbolTable の `function_names` を参照（ラベル生成のため） |
| 公開 API | 文字列ベースの関数名解決を `SymbolTable` 経由で提供 |
| `identifier_map` | semantic_analyzer 内部でのみ使用（ScopeResolver 用に残存） |
| `variable_indices` / `variable_name_to_var_index` | semantic_analyzer 内部でのみ使用（ScopeResolver 用に残存） |

### 次の検討事項

- `identifier_map`, `variable_indices`, `variable_name_to_var_index` の Scope からの除去
  - これらは意味解析中のみ使用。最終的な `Scope` 出力からは不要
  - `ScopeBuilder` の内部フィールドとして留め、`build()` 時に `Scope` に含めない設計が可能
  - ただし `Block.scope` としてネストされたスコープにもこれらが存在するため、影響範囲が大きい
  - 独立したタスクとして切り出すことを推奨
