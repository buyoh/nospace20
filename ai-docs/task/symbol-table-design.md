# デバッグ用シンボルテーブルによる識別子名管理の設計

分離元: [function-args-identifier-resolution.md](./function-args-identifier-resolution.md)

作成日: 2026-02-10

## 動機

`Function.args` の削除に限らず、semantic analyzer の出力全体から文字列識別子を分離し、
インデックスや参照から識別子名を逆引きできる **シンボルテーブル** を別構造体で持つ設計が望ましい。
これにより、ランタイム（interpreter / compiler_ws）は純粋にインデックスベースで動作し、
デバッグ・エラーメッセージ生成時にのみシンボルテーブルを参照する構造になる。

---

## 現状: semantic analyzer 出力に残る文字列の棚卸し

| 構造体 / フィールド | 型 | ランタイムでの使用 |
|---|---|---|
| `Variable.identifier` | `String` | static変数復元時のスロット特定に使用 (exec.rs L220) |
| `Function.args` | `Vec<String>` | `.len()` のみ (compiler_ws) → 削除可能 |
| `Scope.identifier_map` | `BTreeMap<String, Identifier>` | `get_function(name)` で関数検索 (interpreter, compiler_ws) |
| `Scope.variable_indices` | `BTreeMap<String, usize>` | static変数復元 (exec.rs L220)、arg_indices計算 |
| `Scope.variable_name_to_var_index` | `BTreeMap<String, usize>` | ScopeResolver（意味解析中のみ） |
| `Scope.function_names` | `Vec<String>` | 関数イテレーション (interpreter/mod.rs) |
| `ExecExpression::Function` | `String` | 組み込み/ユーザー関数の関数名マッチ (exec.rs L455) |
| `Environment.function_static_storage` | `BTreeMap<String, Vec<i64>>` | 関数名キーで永続ストレージ管理 |

---

## 分析: 各文字列をインデックス化できるか

### (1) Variable.identifier → 削除可能

`Variable.identifier` は現在2つの場面で使用:

1. **`ScopeBuilder.build()` でのマップ構築**: `variable_indices` / `variable_name_to_var_index` の構築時に使用。
   これらのマップ自体をインデックスベースにすれば不要になる。
2. **interpreter の static 変数復元** (exec.rs L220): `variable_indices[&var.identifier]` でスロットインデックスを取得。

static 変数復元は `Variable` に `slot_index: usize` フィールドを直接持たせれば文字列不要:

```rust
// 改善案
pub(crate) struct Variable {
    pub slot_index: usize,  // variable_indices の値を直接保持
    pub is_static: bool,
    pub array_size: Option<usize>,
}
```

これにより `Scope.variable_indices: BTreeMap<String, usize>` も不要になる。

### (2) ExecExpression::Function → 2段階で解決可能

現在 `interpret_call_function` は文字列マッチで組み込み関数とユーザー関数を分岐している:

```rust
match id.as_str() {
    "__puti" => { ... },
    "__putc" => { ... },
    _ => self.interpret_call_user_function(id, args),
}
```

**解決案**: semantic analyzer で関数呼び出しを2種類に分離する:

```rust
pub(crate) enum ExecExpression {
    BuiltinFunction(BuiltinFunctionKind, Vec<Box<ExecExpression>>),
    UserFunction(usize, Vec<Box<ExecExpression>>),  // usize = 関数インデックス
    // ...
}
```

組み込み関数は enum バリアントで、ユーザー関数はインデックスで識別する。
この変更は semantic analyzer での関数呼び出し解決ロジックを拡充すれば実現可能。

### (3) Scope.identifier_map → 段階的に縮小可能

`identifier_map` は `get_function(name)` と `get_variable(name)` で使用。

- **`get_variable`**: 現状は使われていない場面が多い（`ScopeResolver` が代替）。
  テストコードでのみ使用される可能性がある。
- **`get_function`**: interpreter が `"main"` などの名前で関数を取得する。
  これをインデックスベースに変えれば不要。

**解決案**: `get_function` のインターフェースをインデックスベースに変更:

```rust
// 関数名→インデックスの解決は semantic analyzer で完了させる
// interpreter/compiler_ws は直接 scope.functions[idx] でアクセス
impl Scope {
    pub fn get_function_by_index(&self, idx: usize) -> &Function {
        &self.functions[idx]
    }
}
```

`"main"` 関数のエントリポイント解決も semantic analyzer 側で行い、
`Scope` に `main_function_index: Option<usize>` を持たせる。

### (4) function_static_storage → インデックスキー化可能

`BTreeMap<String, Vec<i64>>` を `Vec<Option<Vec<i64>>>` に変更すれば文字列不要:

```rust
// 改善案
pub(crate) function_static_storage: Vec<Option<Vec<i64>>>,
// function_static_storage[func_idx] = Some(storage) or None
```

### (5) Scope.variable_name_to_var_index → 意味解析中のみ使用

`ScopeResolver` で使用されるが、これは semantic analyzer 内部で完結する。
最終的な `Scope` 出力からは除外し、`ScopeBuilder` 専用のフィールドに移動可能。

### (6) Scope.function_names → インデックスイテレーションで代替可能

`function_names` は関数をイテレートするために使用。
関数インデックスベースのイテレーション `0..scope.functions.len()` で代替可能。

---

## 提案: SymbolTable 構造体

```rust
/// デバッグ用シンボルテーブル
/// インデックスから識別子名への逆引きを提供
/// ランタイム動作には不要。デバッグ・エラーメッセージ用
pub struct SymbolTable {
    /// 変数インデックス → 変数名
    pub variable_names: Vec<String>,
    /// 関数インデックス → 関数名
    pub function_names: Vec<String>,
}
```

`Scope` に `debug_symbols: Option<SymbolTable>` として保持し、
リリースビルドやパフォーマンス優先時は `None` にできる設計。

---

## 実現可能性の結論

**実現可能**。ただし段階的な実施を推奨する。

以下の依存関係がある:

```
(1) Variable.identifier 削除
    ← Variable に slot_index を追加
    ← static変数復元をインデックスベースに変更

(2) Function.args 削除          ← 単独で実施可能（function-args-identifier-resolution.md 参照）

(3) ExecExpression::Function のインデックス化
    ← 組み込み関数の enum 化
    ← ユーザー関数の semantic analyzer での解決

(4) Scope.identifier_map の縮小
    ← (3) の完了が必要
    ← get_function のインデックスベース化
    ← main 関数エントリポイント解決の semantic analyzer への移動

(5) function_static_storage のインデックスキー化
    ← (3)(4) の完了が必要

(6) SymbolTable の導入
    ← (1)〜(5) の完了後に文字列情報を SymbolTable に集約
```

---

## 推奨実施順序

1. **(2) Function.args 削除** — 変更量小。単独で完了可能
2. **(1) Variable.identifier → slot_index** — 変更量中。static 変数復元の書き換えが必要
3. **(3) ExecExpression::Function のインデックス化** — 変更量大。組み込み関数 enum の設計が必要
4. **(4)(5)(6) Scope からの文字列排除 + SymbolTable 導入** — (3) の完了後にまとめて実施

各ステップは独立してテスト可能であり、全ステップ完了後に semantic analyzer の出力は
完全にインデックスベースとなり、文字列はオプショナルな SymbolTable のみに集約される。

---

## 関連ドキュメント

- [function-args-identifier-resolution.md](./function-args-identifier-resolution.md) — Function.args の考察（ステップ2）
- [technical-debt.md](./technical-debt.md) — 技術的負債の一覧

---

## 更新履歴

- 2026-02-10: function-args-identifier-resolution.md から分離して作成
- 2026-02-11: ステップ2「Variable.identifier → slot_index」実装完了
  - [variable-identifier-to-slot-index.md](./variable-identifier-to-slot-index.md) に詳細を記録
  - `Variable` に `slot_index` フィールドを追加
  - interpreter で `variable_indices` マップの代わりに `var.slot_index` を使用
  - すべてのテストが成功
