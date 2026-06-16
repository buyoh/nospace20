# エラーメッセージ型の改善

このドキュメントは `CodeParseError.message` を `Cow<'static, str>` に変更する設計の詳細を記載します。

作成日: 2026-02-08  
ステータス: 設計中

## 目次

1. [背景と目的](#1-背景と目的)
2. [現状分析](#2-現状分析)
3. [設計案](#3-設計案)
4. [実装計画](#4-実装計画)
5. [懸念事項とリスク](#5-懸念事項とリスク)

---

## 1. 背景と目的

### 1.1 背景

`CodeParseError` 構造体の `message` フィールドは現在 `String` 型を使用しています。
エラーメッセージの多くは静的な文字列であり、毎回ヒープ割り当てが発生しています。

```rust
// 現在の定義 (src/base/mod.rs)
pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: String, // TODO: consider Cow<'static, str>
    ...
}
```

### 1.2 目的

- **パフォーマンス向上**: 静的文字列の場合、ヒープ割り当てを回避
- **柔軟性の維持**: 動的文字列（変数を含むメッセージ）も対応可能
- **コードの一貫性**: `Cow` パターンを使用した効率的なエラーハンドリング

---

## 2. 現状分析

### 2.1 エラーメッセージの発生箇所

| モジュール | 箇所数 | 主な用途 |
|------------|--------|----------|
| token_parser | 13 | 字句解析エラー（不正文字、エスケープシーケンス等） |
| tree_parser | 8 | 構文解析エラー（予期しないトークン等） |
| semantic_analyzer | 6 | 意味解析エラー（未定義変数、重複定義等） |

### 2.2 エラーメッセージのパターン分類

調査の結果、エラーメッセージは以下の2パターンに分類されます。

#### パターンA: 静的文字列（約60%）

変数を含まない固定のメッセージ。`Cow::Borrowed` で効率化可能。

```rust
// token_parser/mod.rs
"invalid hexadecimal literal: expected at least one hex digit after '0x'".to_string()
"incomplete hex escape sequence: expected 2 hex digits after '\\x'".to_string()
"unexpected end of input in character literal".to_string()
"empty character literal".to_string()
"unclosed character literal".to_string()
"single '&' is not supported yet".to_string()
"single '|' is not supported".to_string()

// tree_parser/macros.rs
"unexpected end of input".to_owned()

// tree_parser/expression/mod.rs, statement/mod.rs
"unexpected comma".to_owned()

// semantic_analyzer/mod.rs
"semantic error: nested function declaration is not supported".to_string()
"semantic error: return statement outside of function".to_string()
"semantic error: continue statement outside of function".to_string()
"semantic error: break statement outside of function".to_string()
```

#### パターンB: 動的文字列（約40%）

変数を含むメッセージ。`Cow::Owned` で対応。

```rust
// token_parser/mod.rs
format!("invalid hex escape sequence: \\x{}", hex_str)
format!("unknown escape sequence: \\{}", c)
format!("expected closing quote, found: {}", c)
format!("invalid char: {}", c)

// tree_parser/macros.rs
format!("unexpected token: expected {}", stringify!($pat))

// semantic_analyzer/mod.rs
format!("undefined variable: {}", v)
format!("semantic error: the name '{}' is already used", name)
```

### 2.3 関連コードの構造

#### `code_parse_error!` マクロ

```rust
// src/base/mod.rs
#[macro_export]
macro_rules! code_parse_error {
    ($ptr: expr, $msg: expr) => {
        CodeParseError::new(Some($ptr), $msg)
    };
    ($msg: expr) => {
        CodeParseError::new(None, $msg)
    };
}
```

#### `CodeParseError::new`

```rust
impl CodeParseError {
    #[track_caller]
    pub fn new(code_pointer: Option<usize>, message: String) -> Self {
        Self {
            code_pointer,
            message,
            #[cfg(debug_assertions)]
            caller: std::panic::Location::caller(),
        }
    }
}
```

---

## 3. 設計案

### 3.1 基本方針

`Cow<'static, str>` を導入し、静的文字列と動的文字列を効率的に扱う。

### 3.2 変更箇所

#### 3.2.1 `CodeParseError` 構造体

```rust
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct CodeParseError {
    pub code_pointer: Option<usize>,
    pub message: Cow<'static, str>, // String から変更
    #[cfg(debug_assertions)]
    pub caller: &'static std::panic::Location<'static>,
}
```

#### 3.2.2 `CodeParseError::new` メソッド

```rust
impl CodeParseError {
    #[track_caller]
    pub fn new(code_pointer: Option<usize>, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code_pointer,
            message: message.into(),
            #[cfg(debug_assertions)]
            caller: std::panic::Location::caller(),
        }
    }
}
```

#### 3.2.3 利便性のためのヘルパーメソッド（オプション）

```rust
impl CodeParseError {
    /// 静的文字列からエラーを生成（ヒープ割り当てなし）
    #[track_caller]
    pub fn from_static(code_pointer: Option<usize>, message: &'static str) -> Self {
        Self::new(code_pointer, Cow::Borrowed(message))
    }
    
    /// 動的文字列からエラーを生成
    #[track_caller]
    pub fn from_string(code_pointer: Option<usize>, message: String) -> Self {
        Self::new(code_pointer, Cow::Owned(message))
    }
}
```

### 3.3 呼び出し側の変更

#### 変更前（静的文字列）

```rust
code_parse_error!(idx, "empty character literal".to_string())
```

#### 変更後（静的文字列）- 推奨A

```rust
code_parse_error!(idx, "empty character literal")
```

#### 変更後（静的文字列）- 推奨B（より明示的）

```rust
CodeParseError::from_static(Some(idx), "empty character literal")
```

#### 変更前（動的文字列）

```rust
code_parse_error!(idx, format!("invalid char: {}", c))
```

#### 変更後（動的文字列）- 変更なし

```rust
code_parse_error!(idx, format!("invalid char: {}", c))
// `format!` は `String` を返し、`Into<Cow<'static, str>>` で自動変換
```

### 3.4 マクロの変更（不要）

`Into<Cow<'static, str>>` を使用することで、マクロ自体の変更は不要です。
以下の呼び出しが全て有効：

- `"static string"` → `Cow::Borrowed`
- `"static string".to_string()` → `Cow::Owned`
- `format!(...)` → `Cow::Owned`

---

## 4. 実装計画

### フェーズ1: 基盤変更

1. `src/base/mod.rs` の `CodeParseError` 構造体を変更
2. `CodeParseError::new` のシグネチャを `impl Into<Cow<'static, str>>` に変更
3. `Clone` derive が引き続き動作することを確認

### フェーズ2: 呼び出し側の最適化

1. **token_parser**: `.to_string()` / `.to_owned()` を削除（静的文字列の場合）
2. **tree_parser**: 同上
3. **semantic_analyzer**: 同上
4. **マクロ** (`match_expect_token`): `.to_owned()` を削除

### フェーズ3: テスト・検証

1. 既存テストの動作確認
2. コンパイル時のエラーがないことを確認
3. ベンチマーク（オプション）

### 作業工数見積もり

| フェーズ | 工数 | 難易度 |
|----------|------|--------|
| フェーズ1 | 小 | 低 |
| フェーズ2 | 中 | 低 |
| フェーズ3 | 小 | 低 |

---

## 5. 懸念事項とリスク

### 5.1 `Clone` の動作

`Cow<'static, str>` は `Clone` trait を実装しているため、既存の `#[derive(Clone)]` は引き続き動作します。

- `Cow::Borrowed(&'static str)` の clone → ポインタのコピー（低コスト）
- `Cow::Owned(String)` の clone → 文字列の完全コピー（既存動作と同じ）

### 5.2 API 互換性

`Into<Cow<'static, str>>` を使用することで、既存コードは変更なしで動作します。

- `String` → `Cow::Owned` に自動変換
- `&'static str` → `Cow::Borrowed` に自動変換

### 5.3 wasm_api への影響

`wasm_api.rs` では `e.message.clone()` で使用されています。

```rust
// src/wasm_api.rs:67
message: e.message.clone(),
```

これは `Cow::clone()` を呼び出すことになり、動作に問題ありません。
ただし、`WasmError.message` は `String` 型なので、明示的な変換が必要になる可能性があります：

```rust
// 変更後
message: e.message.clone().into_owned(),
// または
message: e.message.to_string(),
```

---

## 6. 補足: 他に検討すべき文字列

プロジェクト内で他に `Cow` パターンを適用できる箇所を調査しました。

### 6.1 検討対象外（変更不要）

| 箇所 | 理由 |
|------|------|
| `semantic_analyzer: Variable.identifier` | TODO で `IdentifierInfo` への変更予定 |
| `semantic_analyzer: Function.args` | TODO で identifier_ptr への変更予定 |
| `compiler_ws/encoder.rs: to_string()` | 出力用の文字列生成、最適化不要 |
| `whitespace/interpreter.rs: get_stdout_string()` | 出力用 |
| `compiler_ws/program.rs: to_whitespace(), to_debug_string()` | 出力用 |
| `compiler_ws/instruction.rs: to_mnemonic()` | 出力用 |
| `wasm_api.rs: WasmCompileResult.stdout/output` | 外部API、変更困難 |

### 6.2 結論

現時点で `Cow<'static, str>` への変更が有効なのは `CodeParseError.message` のみです。
他の文字列は出力用または将来的に異なる型への変更が予定されているため、現時点での変更は不要と判断しました。

---

## 関連ドキュメント

- [src/base/mod.rs](../../src/base/mod.rs)
- [src/token_parser/mod.rs](../../src/token_parser/mod.rs)
- [src/tree_parser/macros.rs](../../src/tree_parser/macros.rs)
- [src/semantic_analyzer/mod.rs](../../src/semantic_analyzer/mod.rs)
- [docs-ai/task/technical-debt.md](./technical-debt.md)
