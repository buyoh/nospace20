# 公開 API の設計改善

## 問題 1: 副作用の除去

### `eprintln!` の使用

ライブラリ関数から直接 `eprintln!` で標準エラーに出力しているため、ライブラリとしてのテスタビリティ・再利用性が低下している。

| 関数 | 箇所 | 内容 |
|------|------|------|
| `interpreter::interpret_func` | `src/interpreter/mod.rs` | 関数未発見時に `eprintln!` |
| `interpreter::interpret_all` | `src/interpreter/mod.rs` | `__main` 未発見時に `eprintln!` |

**改善案**: `Option<i64>` ではなく `Result<i64, InterpretError>` を返すようにし、エラーの表示は呼び出し元（CLI / WASM API）で行う。

```rust
pub enum InterpretError {
    FunctionNotFound(String),
    UnexpectedFlow,
    MaxExpressionCountExceeded,
}
```

### `process::exit` の使用

| 関数 | 箇所 | 内容 |
|------|------|------|
| `handle_parse_error` | `src/bin/nospace20.rs` | パースエラー時に `process::exit(1)` |

**改善案**: `Result` を返して `main()` で `process::exit` を呼ぶか、`std::process::Termination` トレイトを活用。

### `panic!` の使用

| 関数 | 箇所 | 内容 |
|------|------|------|
| `interpret_global` | `src/interpreter/mod.rs` | 予期しない制御フローで `panic!` |

**改善案**: `Result` 型に変更し、エラーは呼び出し元に伝播。

## 問題 2: compile_to_whitespace 系 API の爆発

現在 6 つの関数が存在:

1. `compile_to_whitespace_with_options(scope, debug_ext, alloc_ext)`
2. `compile_to_whitespace_with_opt(scope, debug_ext, alloc_ext, opt)`
3. `compile_to_whitespace_debug_with_options(scope, debug_ext, alloc_ext)`
4. `compile_to_whitespace_debug_with_opt(scope, debug_ext, alloc_ext, opt)`
5. (+ さらに増える可能性あり)

`debug`（デバッグニーモニック出力）/ `opt`（最適化オプション）の有無で排他的に関数が増殖。

### 改善案: オプション構造体の統合

```rust
pub struct WsCompileOptions {
    /// デバッグ拡張 API を有効化
    pub debug_ext: bool,
    /// メモリアロケータ拡張を有効化
    pub alloc_ext: bool,
    /// 出力形式
    pub output_format: WsOutputFormat,
    /// 最適化オプション
    pub optimization: OptimizationOptions,
}

pub enum WsOutputFormat {
    /// Whitespace コード（空白文字のみ）
    Whitespace,
    /// デバッグ用ニーモニック
    Mnemonic,
}

pub fn compile_to_whitespace(
    scope: &Scope,
    options: &WsCompileOptions,
) -> Result<String, Vec<CodeParseError>> { ... }
```

## 問題 3: テスト用関数の公開 API 混在

以下のテスト専用関数が `lib.rs` で公開されている:

- `interpret_func_testing`
- `interpret_func_testing_randomize`
- `interpret_func_with_io`

### 改善案

テスト用ヘルパーを `#[cfg(test)]` モジュールまたは `testing` feature flag の下に移動:

```rust
#[cfg(feature = "testing")]
pub mod testing {
    pub fn interpret_func_testing(...) -> BTreeMap<i64, i64> { ... }
    pub fn interpret_func_testing_randomize(...) -> BTreeMap<i64, i64> { ... }
    pub fn interpret_func_with_io(...) -> (BTreeMap<i64, i64>, String) { ... }
}
```

ただし `tests/` ディレクトリの統合テストから呼ばれている点に注意。統合テストは外部クレートとして扱われるため、`#[cfg(test)]` では見えない。`testing` feature を `Cargo.toml` で追加し、テスト時のみ有効化する必要がある。

## 問題 4: `syntactic_analyze` の命名

`lib.rs` で公開されている `syntactic_analyze` 関数は実体が `semantic_analyzer::analyze` を呼び出しているが、名前が「構文解析 (syntactic)」を示唆しており誤解を招く。

**改善案**: `semantic_analyze` にリネーム。後方互換のため `#[deprecated]` 付きの alias を残す。

```rust
pub fn semantic_analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    semantic_analyzer::analyze(root)
}

#[deprecated(note = "Renamed to semantic_analyze")]
pub fn syntactic_analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    semantic_analyze(root)
}
```

## 問題 5: `lib.rs` の冗長ラッパー

`parse_to_tokens` 等が `match Ok(x) => Ok(x), Err(e) => Err(e)` パターンで実質 identity 関数になっている。

**改善案**: 直接委譲に変更。

```rust
// Before
pub fn parse_to_tokens(text: &String) -> Result<Vec<PrettyToken>, Vec<CodeParseError>> {
    match token_parser::parse_to_tokens(text) {
        Ok(x) => Ok(x),
        Err(err) => Err(err),
    }
}

// After
pub fn parse_to_tokens(text: &String) -> Result<Vec<PrettyToken>, Vec<CodeParseError>> {
    token_parser::parse_to_tokens(text)
}
```

## 影響範囲

- `src/lib.rs`
- `src/interpreter/mod.rs` (または `exec.rs`)
- `src/bin/nospace20.rs`
- `src/wasm_api.rs`
- `tests/` 配下の統合テスト
- `Cargo.toml` (testing feature 追加)
