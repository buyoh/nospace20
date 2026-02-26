# WASM版コンパイルエラーの位置情報表示

## 概要

WASM API でコンパイルエラー (`compiler_ws`) が発生した際、エラーの発生位置（行・列）が表示されない問題を解決する。

## 現状分析

### エラー位置が表示されるフェーズ（正常動作）

| フェーズ | モジュール | エラー型 | 位置情報 |
|---------|-----------|---------|---------|
| 字句解析 | `token_parser` | `Vec<CodeParseError>` | `code_pointer: Some(usize)` |
| 構文解析 | `tree_parser` | `Vec<CodeParseError>` | `code_pointer: Some(usize)` |
| 意味解析 | `semantic_analyzer` | `Vec<CodeParseError>` | `code_pointer: Some(usize)` ※一部 None |

WASM API の `convert_errors()` 関数が `CodeParseError.code_pointer` を `TextCode.char_index_to_line()` で `(line, column)` に変換し、`WasmError { line, column }` として返す。

### エラー位置が表示されないフェーズ（問題箇所）

| フェーズ | モジュール | エラー型 | 位置情報 |
|---------|-----------|---------|---------|
| コンパイル | `compiler_ws` | `CompileError` | **なし** |

`compiler_ws` は独自の `CompileError` enum を使用しており、位置情報フィールドがない。

```rust
// src/compiler_ws/mod.rs
pub enum CompileError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}
```

さらに `lib.rs` で `String` に変換されるため、構造化された情報も失われる:

```rust
// src/lib.rs
pub fn compile_to_whitespace(scope: &Scope) -> Result<String, String> {
    compiler_ws::compile(scope)
        .map(|prog| prog.to_whitespace())
        .map_err(|e| e.to_string())  // ← 位置情報消失
}
```

WASM API ではコンパイルエラーを `WasmError { message: err, line: None, column: None }` として返すため、位置が表示されない。

### 位置情報が失われる根本原因

1. `tree_parser` の `Expression` 型が位置情報を持たない（`LocatedStatement` のみ位置あり）
2. `semantic_analyzer` が `ExecStatement` / `ExecExpression` に変換する際、`LocatedStatement.location` を捨てる
3. `compiler_ws` は位置情報なしの `ExecStatement` / `ExecExpression` しか受け取れない

## 設計方針

### 段階的アプローチ

位置情報の粒度を段階的に改善する2フェーズ構成とする。

#### Phase 1: 文レベルの位置情報（本タスクのスコープ）

- `ExecStatement` に `SourceLocation` を付与する
- `compiler_ws` でコンパイルエラー発生時に文レベルの位置情報を活用
- `compile_to_whitespace` の戻り値を構造化エラーに変更
- WASM API でコンパイルエラーに位置情報を表示

#### Phase 2: 式レベルの位置情報

- `Expression` / `ExecExpression` に `SourceLocation` を付与
- より精密なエラー位置の報告
- 詳細設計: [expression-location/](expression-location/)

## 詳細設計 (Phase 1)

### 1. `ExecStatement` に位置情報を追加

`SourceLocation` を保持するよう変更する。

```rust
// src/semantic_analyzer/types.rs

use crate::base::SourceLocation;

pub(crate) struct LocatedExecStatement {
    pub statement: ExecStatement,
    pub location: SourceLocation,
}
```

`Block.statements` の型を `Vec<ExecStatement>` → `Vec<LocatedExecStatement>` に変更:

```rust
pub(crate) struct Block {
    pub scope: super::Scope,
    pub statements: Vec<LocatedExecStatement>,
}
```

`Scope.static_init_statements` と `Scope.root_statements` も同様に変更:

```rust
pub(crate) static_init_statements: Vec<LocatedExecStatement>,
pub(crate) root_statements: Vec<LocatedExecStatement>,
```

### 2. semantic_analyzer での位置情報保持

`LocatedStatement` → `LocatedExecStatement` 変換時に `location` を引き継ぐ。

現在の処理（`src/semantic_analyzer/mod.rs`）:

```rust
for located_stat in statements {
    let stat = &located_stat.statement;
    let loc = &located_stat.location;
    // ... stat を ExecStatement に変換し、loc は捨てる
}
```

変更後:

```rust
for located_stat in statements {
    let stat = &located_stat.statement;
    let loc = &located_stat.location;
    // ... stat を ExecStatement に変換
    let exec_stmt = LocatedExecStatement {
        statement: converted_stmt,
        location: loc.clone(),
    };
}
```

### 3. `CompileError` に位置情報を追加

```rust
// src/compiler_ws/mod.rs

use crate::base::SourceLocation;

#[derive(Debug)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub location: Option<SourceLocation>,
}

#[derive(Debug)]
pub enum CompileErrorKind {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}
```

### 4. `CodeGenContext` に現在の位置トラッキングを追加

コード生成中に処理中の文の位置を追跡する:

```rust
// src/compiler_ws/context.rs

pub struct CodeGenContext<'a> {
    // ... 既存フィールド
    current_location: Option<SourceLocation>,
}

impl<'a> CodeGenContext<'a> {
    pub fn set_location(&mut self, loc: &SourceLocation) {
        self.current_location = Some(loc.clone());
    }

    pub fn current_location(&self) -> Option<SourceLocation> {
        self.current_location.clone()
    }
}
```

`statement.rs` で文を処理する際に位置をセット:

```rust
for located_stmt in &block.statements {
    ctx.set_location(&located_stmt.location);
    program.append(generate_statement(ctx, &located_stmt.statement)?);
}
```

`expression.rs` でエラー生成時に位置を付与:

```rust
return Err(CompileError {
    kind: CompileErrorKind::InvalidOperation("...".to_string()),
    location: ctx.current_location(),
});
```

### 5. `lib.rs` の戻り値型を変更

```rust
// src/lib.rs

// Before:
pub fn compile_to_whitespace(scope: &Scope) -> Result<String, String>

// After:
pub fn compile_to_whitespace(scope: &Scope) -> Result<String, Vec<CodeParseError>>
```

`CompileError` → `CodeParseError` の変換関数を追加:

```rust
fn compile_error_to_code_parse_error(e: CompileError) -> CodeParseError {
    let code_pointer = e.location.map(|loc| loc.start);
    code_parse_error_with_pointer(code_pointer, e.kind.to_string())
}
```

これにより WASM API 側で既存の `convert_errors()` をそのまま利用できる。

### 6. WASM API の更新

`wasm_api.rs` のコンパイルエラー処理を `convert_errors` に統一:

```rust
// Before (compile 関数内):
Err(err) => {
    let result = ResultErr {
        success: false,
        errors: vec![WasmError { message: err, line: None, column: None }],
    };
    ...
}

// After:
Err(errors) => convert_errors(&errors, &text).into(),
```

`WasmWhitespaceVM::new` も同様に変更する。

### 7. 影響を受けるモジュールと変更量

| モジュール | ファイル | 変更内容 | 影響度 |
|-----------|---------|---------|-------|
| `base` | `location.rs` | 変更なし（既存型を使用） | なし |
| `semantic_analyzer` | `types.rs` | `LocatedExecStatement` 追加、`Block`/`Scope` 型変更 | 中 |
| `semantic_analyzer` | `mod.rs` | `LocatedStatement.location` の引き継ぎ | 小 |
| `interpreter` | 各ファイル | `LocatedExecStatement` に合わせたイテレーション変更 | 小 |
| `compiler_ws` | `mod.rs` | `CompileError` → `CompileError` + `CompileErrorKind` | 小 |
| `compiler_ws` | `context.rs` | `current_location` フィールド追加 | 小 |
| `compiler_ws` | `statement.rs` | 位置情報のセット、`LocatedExecStatement` 対応 | 小 |
| `compiler_ws` | `expression.rs` | `CompileError` 生成の更新 | 小 |
| `compiler_ws` | `builtin.rs` | `CompileError` 生成の更新 | 小 |
| `lib.rs` | - | `compile_to_whitespace` 等の戻り値型変更 | 小 |
| `wasm_api.rs` | - | コンパイルエラーハンドリングの統一 | 小 |
| `bin/nospace20.rs` | - | CLI でのコンパイルエラー表示更新 | 小 |
| `tests/` | `compile_test.rs` 等 | エラー型の変更に合わせた修正 | 小 |

### 8. 位置不明のケース

以下のケースでは位置情報が `None` となる（Phase 1 では許容）:

- `MainNotFound`: ソースコード上の特定位置に紐づかない
- 式レベルのエラー: 文の開始位置で代替表示（将来 Phase 2 で改善）

## テスト計画

1. **Unit テスト**: `CompileError` に位置情報が含まれることを確認
2. **WASM テスト**: コンパイルエラーのレスポンスに `line`/`column` が含まれることを確認
3. **Large テスト**: 既存テストケースが通ることを確認（リグレッションなし）

## 作業順序

1. `semantic_analyzer/types.rs`: `LocatedExecStatement` 型の追加、`Block`/`Scope` の型変更
2. `semantic_analyzer/mod.rs`: 位置情報の引き継ぎ
3. `interpreter/`: `LocatedExecStatement` に合わせた変更
4. `compiler_ws/mod.rs`: `CompileError` / `CompileErrorKind` の再設計
5. `compiler_ws/context.rs`: `current_location` の追加
6. `compiler_ws/statement.rs`, `expression.rs`, `builtin.rs`: エラー生成の更新
7. `lib.rs`: 戻り値型の変更と変換関数の追加
8. `wasm_api.rs`: コンパイルエラーハンドリングの統一
9. `bin/nospace20.rs`: CLI のコンパイルエラー表示更新
10. テスト修正・確認

## 進捗

**Phase 1 実装完了**

- ✅ `semantic_analyzer/types.rs`: `LocatedExecStatement` 追加・`Block`/`Scope` 型変更
- ✅ `semantic_analyzer/scope.rs`: `Vec<ExecStatement>` → `Vec<LocatedExecStatement>` 変更
- ✅ `semantic_analyzer/mod.rs`: 位置情報の引き継ぎ実装
- ✅ `interpreter/exec.rs`, `interpreter/mod.rs`: `LocatedExecStatement` 対応
- ✅ `compiler_ws/mod.rs`: `CompileError` → `{CompileErrorKind, CompileError}` 再設計
- ✅ `compiler_ws/context.rs`: `current_location` フィールド・メソッド追加
- ✅ `compiler_ws/statement.rs`, `expression.rs`, `builtin.rs`: エラー生成更新
- ✅ `lib.rs`: `Result<String, String>` → `Result<String, Vec<CodeParseError>>` 変更
- ✅ `wasm_api.rs`: コンパイルエラーハンドリングを `convert_errors()` に統一
- ✅ `bin/nospace20.rs`: `handle_parse_error()` を使用するよう変更
- ✅ テスト修正・全テストパス確認
