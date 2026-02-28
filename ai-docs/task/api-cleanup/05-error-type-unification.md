# エラー型の統一・`src/base/error` モジュール作成

日付: 2026-03-01

## 概要

プロジェクト内に散在する 5 つのエラー型を `src/base/error` モジュールに集約し、ライブラリやCLI に伝達されるエラーの一貫性を確保する。

## 現状の問題

### エラー型の乱立

| エラー型 | 定義場所 | 用途 | `Display` | `Error` |
|----------|----------|------|-----------|---------|
| `CodeParseError` | `src/base/mod.rs` | トークン化・構文解析・意味解析 | 実装済 | 実装済 |
| `CompileError` / `CompileErrorKind` | `src/compiler_ws/mod.rs` | WS コンパイル | 実装済 | 未実装 |
| `InterpretError` | `src/interpreter/mod.rs` | nospace インタプリタ | 実装済 | 実装済 |
| `RuntimeError` | `src/whitespace/interpreter.rs` | WS VM 実行時 | 未実装 | 未実装 |
| `ParseError` | `src/whitespace/parser.rs` | WS パース | 未実装 | 未実装 |

### 主な問題点

1. **`compile_error_to_code_parse_error` 変換で情報喪失**: `CompileErrorKind` の構造化 enum が `to_string()` で文字列化され、位置情報も `SourceLocation`(range) → `Option<usize>`(point) に劣化
2. **パイプラインの型不統一**: parse/analyze は `Vec<CodeParseError>` を返すが、interpret は `InterpretError` 単体を返す
3. **`Display`/`Error` トレイト未実装**: `RuntimeError`, `ParseError` は `Debug` フォーマットでのみ文字列化（WASM API で `format!("{:?}", e)` が使われている）
4. **CLI のエラーハンドリングが分断**: `CodeParseError` は `handle_parse_error()` で整形表示、`InterpretError` は `eprintln!` で簡易表示、`ValidationError` は別パスで処理

## 設計

### 新規モジュール: `src/base/error/`

```
src/base/error/
├── mod.rs              # 統一エラー型 NospaceError, CompileStage, re-export
├── parse_error.rs      # CodeParseError（既存の base/mod.rs から移動）
├── compile_error.rs    # CompileError, CompileErrorKind（compiler_ws/mod.rs から移動）
├── interpret_error.rs  # InterpretError（interpreter/mod.rs から移動）
├── ws_error.rs         # RuntimeError, ParseError（whitespace/ から移動）
└── validation_error.rs # ValidationError（compile_property.rs から移動）
```

### 統一エラー型

ライブラリや CLI に伝達されるエラーを包むエンベロープ型を導入する。

```rust
// src/base/error/mod.rs

/// コンパイルパイプラインのステージ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileStage {
    Tokenize,
    Parse,
    SemanticAnalysis,
    Optimization,
    WsCodeGeneration,
    NospaceExecution,
    WsExecution,
    Validation,
}

/// 統一エラー型
///
/// パイプラインの各ステージから発生するエラーを一つの型で表現する。
/// CLI や WASM API でのエラーハンドリングを統一的に行える。
#[derive(Debug)]
pub enum NospaceError {
    /// トークン化・構文解析・意味解析エラー（複数件）
    Parse(Vec<CodeParseError>),
    /// Whitespace コンパイルエラー
    Compile(CompileError),
    /// nospace インタプリタ実行エラー
    Interpret(InterpretError),
    /// Whitespace VM パースエラー
    WsParse(WsParseError),
    /// Whitespace VM 実行時エラー
    WsRuntime(WsRuntimeError),
    /// 設定バリデーションエラー
    Validation(ValidationError),
}
```

### 各エラー型の移動と改善

#### `CodeParseError`（既存・移動のみ）

`src/base/mod.rs` → `src/base/error/parse_error.rs` に移動。`code_parse_error!` マクロも移動。
既に `Display` + `Error` 実装済みのため変更なし。

#### `CompileError` / `CompileErrorKind`（移動 + `Error` トレイト追加）

`src/compiler_ws/mod.rs` → `src/base/error/compile_error.rs` に移動。

```rust
// src/base/error/compile_error.rs
use crate::base::location::SourceLocation;

#[derive(Debug, Clone)]
pub enum CompileErrorKind {
    UndefinedVariable(String),
    UndefinedFunction(String),
    MainNotFound,
    InvalidOperation(String),
}

#[derive(Debug, Clone)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub location: Option<SourceLocation>,
}

impl std::fmt::Display for CompileError { ... }  // 既存
impl std::error::Error for CompileError {}        // 新規追加
```

これにより `lib.rs` の `compile_error_to_code_parse_error` 変換関数が不要になる。`compile_to_ws` は `Result<String, NospaceError>` または `Result<String, CompileError>` を直接返せる。

#### `InterpretError`（移動のみ）

`src/interpreter/mod.rs` → `src/base/error/interpret_error.rs` に移動。
既に `Display` + `Error` 実装済みのため変更なし。

#### `RuntimeError`（移動 + リネーム + `Display`/`Error` 追加）

`src/whitespace/interpreter.rs` → `src/base/error/ws_error.rs` に移動。
名前衝突を避けるため `WsRuntimeError` にリネーム。

```rust
// src/base/error/ws_error.rs

#[derive(Debug, Clone)]
pub enum WsRuntimeError {
    StackUnderflow,
    DivisionByZero,
    UndefinedLabel(i64),
    UninitializedHeap(i64),
    CallStackUnderflow,
    ProgramCounterOutOfBounds,
    IoError(String),
    AssertionFailed(i64),
}

impl std::fmt::Display for WsRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StackUnderflow => write!(f, "stack underflow"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::UndefinedLabel(id) => write!(f, "undefined label: {}", id),
            Self::UninitializedHeap(addr) => write!(f, "uninitialized heap at address {}", addr),
            Self::CallStackUnderflow => write!(f, "call stack underflow"),
            Self::ProgramCounterOutOfBounds => write!(f, "program counter out of bounds"),
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
            Self::AssertionFailed(val) => write!(f, "assertion failed: {}", val),
        }
    }
}

impl std::error::Error for WsRuntimeError {}
```

#### `ParseError`（移動 + リネーム + `Display`/`Error` 追加）

`src/whitespace/parser.rs` → `src/base/error/ws_error.rs` に移動。
`WsParseError` にリネーム。

```rust
#[derive(Debug, Clone)]
pub enum WsParseError {
    InvalidImp { position: usize },
    InvalidCommand { position: usize, imp: String },
    UnexpectedEof { context: String },
    InvalidNumber { position: usize },
    InvalidLabel { position: usize },
    DuplicateLabel { label_id: i64, first_position: usize, second_position: usize },
}

impl std::fmt::Display for WsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidImp { position } =>
                write!(f, "invalid IMP at position {}", position),
            Self::InvalidCommand { position, imp } =>
                write!(f, "invalid command for IMP '{}' at position {}", imp, position),
            Self::UnexpectedEof { context } =>
                write!(f, "unexpected end of file while parsing {}", context),
            Self::InvalidNumber { position } =>
                write!(f, "invalid number at position {}", position),
            Self::InvalidLabel { position } =>
                write!(f, "invalid label at position {}", position),
            Self::DuplicateLabel { label_id, first_position, second_position } =>
                write!(f, "duplicate label {} (first at {}, second at {})",
                    label_id, first_position, second_position),
        }
    }
}

impl std::error::Error for WsParseError {}
```

#### `ValidationError`（移動のみ）

`src/compile_property.rs` → `src/base/error/validation_error.rs` に移動。
既に `Display` + `Error` 実装済み。

### `NospaceError` のトレイト実装

```rust
impl std::fmt::Display for NospaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(errors) => {
                for (i, e) in errors.iter().enumerate() {
                    if i > 0 { writeln!(f)?; }
                    write!(f, "{}", e)?;
                }
                Ok(())
            }
            Self::Compile(e) => write!(f, "{}", e),
            Self::Interpret(e) => write!(f, "{}", e),
            Self::WsParse(e) => write!(f, "{}", e),
            Self::WsRuntime(e) => write!(f, "{}", e),
            Self::Validation(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for NospaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(e) => Some(e),
            Self::Interpret(e) => Some(e),
            Self::WsParse(e) => Some(e),
            Self::WsRuntime(e) => Some(e),
            Self::Validation(e) => Some(e),
            Self::Parse(_) => None,  // Vec<CodeParseError> は単一の source にならない
        }
    }
}

// 各エラー型からの From 実装
impl From<Vec<CodeParseError>> for NospaceError { ... }
impl From<CompileError> for NospaceError { ... }
impl From<InterpretError> for NospaceError { ... }
impl From<WsParseError> for NospaceError { ... }
impl From<WsRuntimeError> for NospaceError { ... }
impl From<ValidationError> for NospaceError { ... }
```

### `lib.rs` の API 更新

`compile_to_ws` の戻り値を `Result<String, CompileError>` に変更し、`compile_error_to_code_parse_error` 変換関数を削除。

```rust
// Before
pub fn compile_to_ws(
    scope: &Scope,
    options: &WsCompileOptions,
) -> Result<String, Vec<CodeParseError>> {
    ...
    .map_err(|e| vec![compile_error_to_code_parse_error(e)])?;
    ...
}

// After
pub fn compile_to_ws(
    scope: &Scope,
    options: &WsCompileOptions,
) -> Result<String, CompileError> {
    ...
}
```

CLI や WASM API 側では、`NospaceError` への変換は `From` トレイトで自動的に行える。

## `base` モジュールの最終構成

```
src/base/
├── mod.rs              # pub mod error, pub mod location, ...
├── error/
│   ├── mod.rs          # NospaceError, CompileStage, re-export
│   ├── parse_error.rs  # CodeParseError, code_parse_error! マクロ
│   ├── compile_error.rs # CompileError, CompileErrorKind
│   ├── interpret_error.rs # InterpretError
│   ├── ws_error.rs     # WsRuntimeError, WsParseError
│   └── validation_error.rs # ValidationError
├── location.rs         # SourceLocation (既存)
├── shared_writer.rs    # SharedWriter (既存)
├── pure_eval.rs        # (既存)
└── constexpr_eval.rs   # (既存)
```

## 作業ステップ

### Step 1: `src/base/error/` ディレクトリ・モジュール作成

1. `src/base/error/mod.rs` 作成 — re-export のみ
2. `src/base/error/parse_error.rs` 作成 — `base/mod.rs` から `CodeParseError` + `code_parse_error!` マクロを移動
3. `src/base/mod.rs` 更新 — `pub mod error` 追加、`CodeParseError` の re-export パス変更
4. `cargo build` で既存コードが壊れないことを確認

### Step 2: `CompileError` の移動

1. `src/base/error/compile_error.rs` 作成
2. `compiler_ws/mod.rs` から `CompileError`, `CompileErrorKind` を移動
3. `compiler_ws/mod.rs` で `pub use crate::base::error::compile_error::*` を追加（後方互換）
4. `std::error::Error` トレイト実装追加
5. `cargo build` 確認

### Step 3: `InterpretError` の移動

1. `src/base/error/interpret_error.rs` 作成
2. `interpreter/mod.rs` から移動
3. `interpreter/mod.rs` で re-export
4. `cargo build` 確認

### Step 4: Whitespace エラー型の移動・リネーム

1. `src/base/error/ws_error.rs` 作成
2. `whitespace/interpreter.rs` から `RuntimeError` → `WsRuntimeError` として移動
3. `whitespace/parser.rs` から `ParseError` → `WsParseError` として移動
4. `Display` + `Error` トレイト実装追加
5. 元のモジュールで type alias を残すか re-export で後方互換維持
6. `cargo build` 確認

### Step 5: `ValidationError` の移動

1. `src/base/error/validation_error.rs` 作成
2. `compile_property.rs` から移動
3. `compile_property.rs` で re-export
4. `cargo build` 確認

### Step 6: `NospaceError` 統一型の作成

1. `src/base/error/mod.rs` に `NospaceError` enum, `CompileStage`, `From` 実装を追加
2. `lib.rs` で `pub use base::error::NospaceError` を公開

### Step 7: `compile_to_ws` の戻り値型変更

1. `lib.rs` の `compile_to_ws` を `Result<String, CompileError>` に変更
2. `compile_error_to_code_parse_error` 関数を削除
3. CLI (`bin/nospace20.rs`) のエラーハンドリングを `NospaceError` ベースに更新
4. WASM API のエラーハンドリングを更新

### Step 8: テスト確認

```bash
cargo test
cargo build --features wasm --target wasm32-unknown-unknown
```

## 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/base/mod.rs` | `pub mod error` 追加、`CodeParseError` の定義を移動 |
| `src/base/error/` | **新規ディレクトリ** — 6 ファイル作成 |
| `src/compiler_ws/mod.rs` | `CompileError` 定義を移動、re-export 追加 |
| `src/interpreter/mod.rs` | `InterpretError` 定義を移動、re-export 追加 |
| `src/whitespace/interpreter.rs` | `RuntimeError` 定義を移動、re-export / alias |
| `src/whitespace/parser.rs` | `ParseError` 定義を移動、re-export / alias |
| `src/compile_property.rs` | `ValidationError` 定義を移動、re-export |
| `src/lib.rs` | `NospaceError` 公開、`compile_error_to_code_parse_error` 削除 |
| `src/bin/nospace20.rs` | エラーハンドリング更新 |
| `src/wasm_api/` | エラー変換パス更新（`Display` 使用に移行） |

## 作業見積もり

中〜大。型定義の移動は機械的だが、以下の点で慎重さが必要:

- `code_parse_error!` マクロの移動（`#[track_caller]` を含む）
- re-export による後方互換性の維持
- CLI/WASM API のエラーハンドリング書き換え
- `compile_to_ws` の戻り値型変更による影響波及

## 備考

- Step 1～5 は各エラー型を独立して移動できるため、段階的に実行可能
- Step 6（`NospaceError` 導入）は Step 1～5 完了後に実施
- Step 7（`compile_to_ws` 戻り値変更）は 03-deprecated-migration タスクの deprecated 関数削除後に実施するのが望ましい（deprecated 関数の戻り値型を変更する必要がなくなるため）
- WS エラー型のリネーム（`RuntimeError` → `WsRuntimeError`）は、元のモジュールで `pub type RuntimeError = WsRuntimeError` として一時的に互換性を維持できる
