# Rust イディオム・型安全性の改善

## 進捗

- [x] edition 2021 への移行（`cargo fix --edition` + Cargo.toml 変更）
- [x] `assert_matches` を `dev-dependencies` に移動
- [x] `SourceLocation` に `Copy`, `PartialEq`, `Eq` トレイトを追加
- [x] `bool_to_int` の可視性を `pub` → `pub(crate)` に変更
- [x] エラー表示で残りのエラー数を通知
- [x] 整数リテラルのオーバーフロー検出（`checked_mul`/`checked_add`）
- [x] `ConstexprEnv::assign_variable` の `contains_key` + `insert` を `get_mut` に改善

### 未着手（大規模変更のため後回し）

- [ ] リリースプロファイルの分離（wasm-pack が custom profile をサポートしないため保留）
- [ ] `PrettyToken` の構造体化（影響範囲が広い）
- [ ] `Statement` バリアントの構造体化（影響範囲が広い）
- [ ] `target_extensions` の `HashSet` 化
- [ ] `Token` / `Expression` への `PartialEq` 追加
- [ ] `logger` → `source_map` リネーム

## Cargo.toml

### edition の更新

現在 `edition = "2018"` を使用しているが、`edition = "2021"` に移行すべき。

メリット:
- `use` パスの解決が簡潔化（`crate::` プレフィックスの省略が可能）
- `IntoIterator for arrays` の自動実装
- `panic!` のフォーマットが改善（意図しない `panic!("{}")` を防止）
- Disjoint capture in closures（クロージャのキャプチャがフィールド単位に）

**作業量**: 小。ほとんどの場合コード変更不要。`cargo fix --edition` で自動移行可能。

### assert_matches の依存位置

`assert_matches` が `[dependencies]` に記載されているが、テスト専用のため `[dev-dependencies]` に移動すべき。

```toml
# Before
[dependencies]
assert_matches = "1.5"

# After
[dev-dependencies]
assert_matches = "1.5"
```

`lib.rs` の `#[cfg(test)] #[macro_use] extern crate assert_matches;` により、テスト以外ではコンパイルされないが、`cargo` の依存解決に影響する可能性がある。

### リリースプロファイルの分離

現在 `[profile.release]` に `opt-level = "z"` が設定されており、WASM サイズ最適化のためだが、CLI バイナリの実行速度も犠牲にしている。

```toml
# CLI 用リリースプロファイル（デフォルト）
[profile.release]
opt-level = 2

# WASM 用リリースプロファイル
[profile.release-wasm]
inherits = "release"
opt-level = "z"
```

`build-wasm.sh` で `--profile release-wasm` を指定する形に変更。

## 型設計

### PrettyToken のタプル型 → 構造体

```rust
// Before
pub type PrettyToken = (Token, TokenInfo);

// After
pub struct PrettyToken {
    pub token: Token,
    pub info: TokenInfo,
}
```

タプルではフィールドの意味が `.0` / `.1` で不明瞭。構造体にすると可読性が向上し、将来のフィールド追加も容易。

**影響範囲**: token_parser, tree_parser, lib.rs — タプルのデストラクチャリングを構造体パターンに書き換える必要がある。中規模の変更。

### Statement バリアントの構造体化

```rust
// Before
VariableDeclaration(String, Box<LocatedExpression>, bool, bool, Option<i64>)

// After
VariableDeclaration {
    name: String,
    init_expr: Box<LocatedExpression>,
    is_final: bool,
    is_const: bool,
    constexpr_value: Option<i64>,
}
```

5 フィールドの名前なしタプルバリアントは可読性が極めて低い。

### SourceLocation に Copy トレイトを追加

`SourceLocation` は `(usize, usize)` 相当の軽量な構造体だが、`Copy` が派生されていないため毎回 `.clone()` が必要。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub start: usize,
    pub end: usize,
}
```

### target_extensions の型

`CompileProperty::target_extensions` が `Vec<TargetExtension>` だが、重複排除・検索効率の観点から `HashSet<TargetExtension>` が適切。`TargetExtension` には既に `Hash` が派生されている。

```rust
pub target_extensions: HashSet<TargetExtension>,
```

## 命名

### syntactic_analyze → semantic_analyze

前述（[04-api-design.md](04-api-design.md#問題-4-syntactic_analyze-の命名)）。

### logger モジュール → source_map

`src/logger/mod.rs` は `TextCode`（ソースコード位置計算）を提供するが、ロギングとは無関係。

```
// Before
src/logger/mod.rs  // TextCode: バイト位置 → (行, 列)

// After
src/source_map/mod.rs  // TextCode or SourceMap: バイト位置 → (行, 列)
```

### bool_to_int の可視性

`src/base/pure_eval.rs` の `bool_to_int` が `pub` だが、crate 外に公開する必要はない。

```rust
// Before
pub fn bool_to_int(b: bool) -> i64 { ... }

// After
pub(crate) fn bool_to_int(b: bool) -> i64 { ... }
```

## エラー表示の限定

`src/bin/nospace20.rs` でエラーを `take(3)` で最大 3 件のみ表示しているが、残りのエラー数を通知していない。

```rust
// 改善案
let errors: Vec<_> = errors.collect();
let displayed = errors.iter().take(3);
for err in displayed { /* 表示 */ }
if errors.len() > 3 {
    eprintln!("... and {} more errors", errors.len() - 3);
}
```

## 整数オーバーフロー対策

`token_parser::parse_number` で `value = value * 10 + d as i64` がオーバーフロー時にパニック（debug）または無言 wrap（release）する。

```rust
// 改善案
value = value.checked_mul(10)
    .and_then(|v| v.checked_add(d as i64))
    .ok_or_else(|| code_parse_error!(pos, "integer literal overflow"))?;
```

## ConstexprEnv の entry API 活用

`src/base/constexpr_eval.rs` の `assign_variable` で `contains_key` → `insert` の 2 回ハッシュ計算が発生。

```rust
// Before
if self.scopes.last().unwrap().contains_key(name) {
    self.scopes.last_mut().unwrap().insert(name.to_string(), value);
} else { ... }

// After
match self.scopes.last_mut().unwrap().entry(name.to_string()) {
    Entry::Occupied(mut e) => { e.insert(value); Ok(()) }
    Entry::Vacant(_) => { /* 上位スコープ検索 */ }
}
```

## Token / Expression への PartialEq 追加

テスタビリティ向上のため、`Token` と `Expression` に `PartialEq` を派生実装。

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Token { ... }
```

`Expression::Invalid(usize)` 等の内部インデックスは比較困難だが、テスト用途では `PartialEq` があると AST 構造の比較が容易になる。
