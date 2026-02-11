# デバッグ用シンボルテーブルによる識別子名管理の設計

作成日: 2026-02-10
**ステータス**: 🚧 進行中（ステップ1-4完了、ステップ5-6未完了）

## 実装状況

| ステップ | 内容 | ステータス | 完了日 | 詳細 |
|---------|------|-----------|--------|------|
| 1 | Function.args 削除 | ✅ 完了 | 2026-02-10 | [function-args-identifier-resolution-completed.md](../done-task/function-args-identifier-resolution-completed.md) |
| 2 | Variable.identifier → slot_index | ✅ 完了 | 2026-02-11 | [variable-identifier-to-slot-index.md](../done-task/variable-identifier-to-slot-index.md) |
| 3 | ExecExpression::Function のインデックス化 | ✅ 完了 | 2026-02-11 | [builtin-function-indexing.md](../done-task/builtin-function-indexing.md) |
| 4 | Scope.identifier_map の縮小 | ✅ 完了 | 2026-02-11 | [main-function-indexing.md](../done-task/main-function-indexing.md) |
| 5 | function_static_storage のインデックスキー化 | ⏳ 未着手 | - | 本ドキュメント参照 |
| 6 | SymbolTable の導入 | ⏳ 未着手 | - | 本ドキュメント参照 |
（2026-02-11更新）

| 構造体 / フィールド | 型 | ステータス | 備考 |
|---|---|---|---|
| ~~`Variable.identifier`~~ | ~~`String`~~ | ✅ 削除済 | ステップ2で `slot_index` に置き換え |
| ~~`Function.args`~~ | ~~`Vec<String>`~~ | ✅ 削除済 | ステップ1で削除 |
| `Scope.identifier_map` | `BTreeMap<String, Identifier>` | ⏳ 残存 | **ステップ4で対応予定** |
| ~~`Scope.variable_indices`~~ | ~~`BTreeMap<String, usize>`~~ | ✅ 削除済 | ステップ2で不要に |
| `Scope.variable_name_to_var_index` | `BTreeMap<String, usize>` | ⚠️ 内部のみ | 意味解析中のみ使用 |
| `Scope.function_names` | `Vec<String>` | ⏳ 残存 | **ステップ4で対応予定** |
| ~~`ExecExpression::Function`~~ | ~~`String`~~ | ✅ 削除済 | ステップ3で enum 化 |
| `Environment.function_static_storage` | `BTreeMap<String, Vec<i64>>` | ⏳ 残存 | **ステップ5で対応予定** |

---

## 完了済みステップの要約

### ✅ ステップ1: Function.args 削除（完了）

詳細: [function-args-identifier-resolution-completed.md](../done-task/function-args-identifier-resolution-completed.md)

- `Function.args` フィールドを削除
- 引数の情報は `arg_indices` のみでインデックスベースで管理

### ✅ ステップ2: Variable.identifier → slot_index（完了）

詳細: [variable-identifier-to-slot-index.md](../done-task/variable-identifier-to-slot-index.md)

- `Variable` に `slot_index` フィールドを追加
- `Scope.variable_indices` マップが不要に
- static 変数復元がインデックスベースに

### ✅ ステップ3: ExecExpression::Function のインデックス化（完了）

詳細: [builtin-function-indexing.md](../done-task/builtin-function-indexing.md)

- `BuiltinFunctionKind` enum を定義
- `ExecExpression::BuiltinFunction` が enum ベースに
- 組み込み関数の文字列比較が不要に

---

## 残りのステップの詳細設計

### ⏳ ステップ4: Scope.identifier_map の縮小
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

**現状**: `identifier_map` は `get_function(name)` で関数を名前から検索するために使用されている。

**課題**:
- interpreter が `"main"` などの関数名で関数を取得している
- ランタイムで文字列マッチングが発生

**解決案**:

1. `Scope` に `main_function_index: Option<usize>` を追加
2. semantic analyzer が `"main"` 関数のインデックスを事前に解決
3. interpreter/compiler_ws は `scope.functions[main_idx]` で直接アクセス
4. `get_function(name)` メソッドを削除または非推奨化

**実装の影響**:
- interpreter/mod.rs の main 関数取得処理を変更
- テストコードで `get_function` を使用している箇所の修正が必要

**依存関係**: ステップ3（組み込み関数のインデックス化）の完了が前提

### ⏳ ステップ5: function_static_storage のインデックスキー化
---

## 提案: SymbolTable 構造体
**目的**: ステップ1-5で削除された全ての文字列情報を、デバッグ用の別構造体に集約する。

**設計**:

```rust
/// デバッグ用シンボルテーブル
/// インデックスから識別子名への逆引きを提供
/// ランタイム動作には不要。デバッグ・エラーメッセージ用
pub struct SymbolTable {
    /// 変数スロットインデックス → 変数名
    pub variable_names: Vec<Option<String>>,
    /// 関数インデックス → 関数名
    pub function_names: Vec<String>,
}

/// Scope に追加
pub struct Scope {
    // ... 既存のフィールド ...
    /// デバッグ用シンボルテーブル（オプショナル）
    pub debug_symbols: Option<SymbolTable>,
}
```

**利用シーン**:
- エラーメッセージで変数名・関数名を表示
- デバッグログで識別子名を表示
- テストコードでの検証

**実装方針**:
- semantic analyzer が SymbolTable を構築
- `--release` ビルドや `--no-debug-symbols` フラグで `None` に設定可能
- ランタイムコードは SymbolTable に一切依存しないインデックスベース設計を維持

**依存関係**: ステップ1-5の全てが完了していることが前提

---

## 次のステップ

ステップ4から順次実装を進める:

1. **ステップ4**: Scope.identifier_map の縮小と main 関数インデックス化
   - 変更量: 中
   - 影響範囲: interpreter/mod.rs, テストコード

2. **ステップ5**: function_static_storage のインデックスキー化
   - 変更量: 小
   - 影響範囲: interpreter/exec.rs

3. **ステップ6**: SymbolTable の導入
   - 変更量: 中
   - 影響範囲: semantic analyzer, エラーメッセージ生成箇所

全ステップ完了後、semantic analyzer の出力は完全にインデックスベースとなり、
文字列はオプショナルな SymbolTable のみに集約される。

---

## 関連ドキュメント

### 完了済み
- [function-args-identifier-resolution-completed.md](../done-task/function-args-identifier-resolution-completed.md) — ステップ1
- [variable-identifier-to-slot-index.md](../done-task/variable-identifier-to-slot-index.md) — ステップ2
- [builtin-function-indexing.md](../done-task/builtin-function-indexing.md) — ステップ3

### 参考
- [technical-debt.md](../done-task

## 関連ドキュメント

- [function-args-identifier-resolution.md](./function-args-identifier-resolution.md) — Function.args の考察（ステップ2）
- [technical-debt.md](./technical-debt.md) — 技術的負債の一覧

---

## 更新履歴

- 2026-02-10: 設計ドキュメントとして作成
- 2026-02-11: ステップ1「Function.args 削除」実装完了
  - [function-args-identifier-resolution-completed.md](../done-task/function-args-identifier-resolution-completed.md) に詳細を記録
- 2026-02-11: ステップ2「Variable.identifier → slot_index」実装完了
  - [variable-identifier-to-slot-index.md](../done-task/variable-identifier-to-slot-index.md) に詳細を記録
  - `Variable` に `slot_index` フィールドを追加
  - interpreter で `variable_indices` マップの代わりに `var.slot_index` を使用
- 2026-02-11: ステップ3「ExecExpression::Function のインデックス化」実装完了
  - [builtin-function-indexing.md](../done-task/builtin-function-indexing.md) に詳細を記録
  - `BuiltinFunctionKind` enum を定義
  - 組み込み関数の識別を文字列マッチングから enum ベースに変更
  - interpreter と compiler_ws で文字列比較が不要に
- 2026-02-11: ステップ4「Scope.identifier_map の縮小」実装完了
  - [main-function-indexing.md](../done-task/main-function-indexing.md) に詳細を記録
  - `Scope` に `main_function_index` フィールドを追加
  - main 関数の取得をインデックスベースに変更
  - `initialize_function_statics` を直接イテレーションに変更
  - interpreter と compiler_ws で main 関数の文字列検索が不要に
- 2026-02-11: ドキュメント整理
  - 完了済みステップ（1-3）と未完了ステップ（4-6）を明確に区別
  - 実装状況テーブルを追加
  - 完了済みステップの詳細説明を簡略化し done-task へのリンクを強化
  - 残りのステップ（4-6）の詳細設計を展開
