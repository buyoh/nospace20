# エラー型の統一・エラーハンドリング改善

## 進捗

- [x] B: `CodeParseError` に `Display` / `Error` トレイトを実装
- [x] D: `CompileProperty::validate()` のエラー型を `String` → `ValidationError` に改善
- [x] ユニットテスト追加（Display 表示、std::error::Error 準拠）

### 未着手（大規模変更のため後回し）

- [ ] A: 統一エラー型 `NospaceError` の導入（全モジュールに影響する大規模リファクタリング）
- [ ] C: エラー収集ポリシーの統一（multi-error-reporting タスクと統合）

## 現状の問題

### 問題 1: エラー型の乱立

現在、以下のエラー型が混在している:

| エラー型 | 使用箇所 | 備考 |
|----------|----------|------|
| `CodeParseError` | token_parser, tree_parser, semantic_analyzer | 位置情報 + メッセージ |
| `CompileError` / `CompileErrorKind` | compiler_ws | コンパイラ固有エラー |
| `String` | `CompileProperty::validate()` | 型安全でない |
| `ParseError` | whitespace パーサ | WS パース固有 |
| `RuntimeError` | whitespace VM | 実行時エラー |

`lib.rs` の `compile_to_whitespace_*` 系関数で `CompileError` を `Vec<CodeParseError>` に変換する際、エラーの構造情報（`CompileErrorKind` のバリアント）が失われる。

### 問題 2: エラー収集ポリシーの不統一

- `semantic_analyzer`: 一部の関数は最初のエラーで即座に `Err` を返し、一部は `errors` ベクタに `append` して最後にまとめて返す
- `tree_parser`: エラーがあれば全体を `Err` にする（部分的な AST + エラーの返却は非対応）
- `token_parser`: エラーは即座に返す

### 問題 3: `CodeParseError` に `Display` / `Error` トレイトが未実装

`std::error::Error` を実装していないため、`?` 演算子での利便性が低く、他のエラーハンドリングクレート（`anyhow` 等）との統合ができない。

## 改善案

### A: 統一エラー型の導入

```rust
/// コンパイルステージを表す列挙型
pub enum CompileStage {
    Tokenize,
    Parse,
    SemanticAnalysis,
    Optimization,
    CodeGeneration,
    Runtime,
}

/// 統一エラー型
pub struct NospaceError {
    pub stage: CompileStage,
    pub location: Option<SourceLocation>,
    pub message: Cow<'static, str>,
    pub kind: ErrorKind,  // 各ステージ固有の分類
}
```

ただし、既存の `CodeParseError` を使う外部 API（テスト、WASM）が多数あるため、段階的な移行が必要。

### B: `Display` / `Error` トレイトの実装

```rust
impl std::fmt::Display for CodeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // code_pointer と message を組み合わせて表示
    }
}

impl std::error::Error for CodeParseError {}
```

### C: エラー収集ポリシーの統一

`semantic_analyzer` のエラー収集を統一する。方針の選択肢:

1. **最初のエラーで中断** — シンプルだがユーザー体験が悪い
2. **全エラーを収集** — ユーザー体験は良いが実装が複雑
3. **回復可能なエラーのみ収集** — バランスが良いが判断基準が曖昧

推奨: 方針 3 を採用し、各エラーに `recoverable: bool` フラグを追加。致命的なエラー（スコープ構造の不整合等）では即座に中断し、局所的なエラー（未定義変数参照等）では収集を続行。

### D: `CompileProperty::validate()` のエラー型改善

`Result<(), String>` → `Result<(), ValidationError>` に変更。

```rust
pub enum ValidationError {
    UnsupportedStd(LanguageStd),
    IncompatibleOptions { target: CompileTarget, std: LanguageStd },
    UnimplementedFeature(String),
}
```

## 影響範囲

- `src/base/mod.rs`
- `src/lib.rs`
- `src/compiler_ws/mod.rs`
- `src/semantic_analyzer/mod.rs`
- `src/compile_property.rs`
- `src/bin/nospace20.rs`
- `src/wasm_api.rs`
- `tests/` 配下のテストコード

## 作業見積もり

- A (統一エラー型): 大 — 全モジュールに影響する大規模リファクタリング
- B (Display/Error): 小 — `base/mod.rs` のみ
- C (収集ポリシー): 中 — 既存タスク [multi-error-reporting.md](../multi-error-reporting.md) と統合可能
- D (validate エラー型): 小 — `compile_property.rs` と `bin/nospace20.rs`
