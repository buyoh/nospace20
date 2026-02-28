# コード重複の解消

## 重複箇所一覧

### 1. SharedWriter 構造体

**箇所**: `src/lib.rs` (`interpret_func_with_io`) と `src/wasm_api.rs`

両方に `Rc<RefCell<Vec<u8>>>` をラップする `SharedWriter` 構造体が定義されている。

```rust
struct SharedWriter(Rc<RefCell<Vec<u8>>>);
impl std::io::Write for SharedWriter { ... }
```

**改善案**: `src/interpreter/types.rs` または `src/base/` に共通の `SharedWriter` を配置。

### 2. constexpr 評価ロジック

**箇所**: 
- `src/semantic_analyzer/mod.rs` — `evaluate_constexpr_expr`, `evaluate_constexpr_by_name`
- `src/base/constexpr_eval.rs` — `eval_constexpr_expr`, `ConstexprEnv`

コアの式評価ロジック（短絡評価、演算子マッチング、二項/単項演算の評価）が 2 箇所に存在。
`semantic_analyzer` 側は raw/resolved/evaluating の 3 テーブル版、`constexpr_eval` 側は `ConstexprEnv` 版という違いがあるが、式評価のコアは同一。

**改善案**: 
- `constexpr_eval.rs` の `eval_constexpr_expr` を唯一の式評価器とする
- `semantic_analyzer` 側は `ConstexprEnv` を構築して委譲する形に変更
- 3 テーブルの管理は `ConstexprEnv` のコンストラクタで変換

### 3. randomize_uninit パターン

**箇所**: `src/interpreter/` 内の初期化コード（4 箇所）

```rust
if env.config.randomize_uninit {
    // ランダム値で初期化
} else {
    vec![0; size]
}
```

**改善案**: ヘルパー関数 `Environment::create_initial_values(size: usize) -> Vec<i64>` を追加。

### 4. エスケープシーケンスパーサ

**箇所**: `src/token_parser/mod.rs` 内の `parse_char_literal` と `parse_string_literal`

同一のエスケープ処理（`\n`, `\t`, `\\`, `\0`, `\xHH` 等）が 2 関数で繰り返されている。

**改善案**: `fn parse_escape_sequence(chars: &[char], pos: &mut usize) -> Result<char, ...>` を抽出。

### 5. WASM コンパイルパイプライン

**箇所**: `src/wasm_api.rs` 内の `run`, `compile`, `parse`, `WasmWhitespaceVM::new`

各関数で以下が手動展開されている:
```rust
let tokens = parse_to_tokens(&source)?;
let tree = parse_to_tree(&tokens)?;
let mut scope = syntactic_analyze(&tree)?;
optimize(&mut scope, &opt);
```

**改善案**: 共通の `compile_pipeline` ヘルパーに抽出。

```rust
fn compile_pipeline(
    source: &str,
    opt: &OptimizationOptions,
) -> Result<Scope, Vec<CodeParseError>> {
    let tokens = parse_to_tokens(&source.to_string())?;
    let tree = parse_to_tree(&tokens)?;
    let mut scope = syntactic_analyze(&tree)?;
    optimize(&mut scope, opt);
    Ok(scope)
}
```

### 6. Store/Retrieve のコード生成 (compiler_ws)

**箇所**: `src/compiler_ws/expression.rs`
- `generate_store_variable` + `generate_store_variable_void` (約 100 行ずつ)
- `generate_store_array` + `generate_store_array_void` (約 100 行ずつ)

通常版と void 版の違いは末尾の `Retrieve` 命令の有無のみ。

**改善案**: `void_context: bool` パラメータで統合。

```rust
fn generate_store_variable(
    &self, ctx: &mut CodeGenContext, ..., void_context: bool
) -> Result<WsProgram, CompileError> {
    // ...共通ロジック...
    if !void_context {
        prog.push(Instruction::Retrieve);
    }
    Ok(prog)
}
```

## 影響度

| 重複 | ファイル数 | 削減見込み | 優先度 |
|------|-----------|-----------|--------|
| SharedWriter | 2 | 約 20 行 | Low |
| constexpr 評価 | 2 | 約 80 行 | Medium |
| randomize_uninit | 1 | 約 20 行 | Low |
| エスケープシーケンス | 1 | 約 30 行 | Low |
| WASM パイプライン | 1 | 約 40 行 | Medium |
| Store/Retrieve | 1 | 約 200 行 | Medium |

## Progress

### 実施済み

- **SharedWriter**: `src/base/shared_writer.rs` に共通の `SharedWriter` 構造体を配置。`src/lib.rs` と `src/wasm_api/whitespace_vm.rs` から共通モジュールを参照するように変更。
- **randomize_uninit**: `exec::create_uninit_vec(size, randomize)` ヘルパー関数を追加。`src/interpreter/exec.rs` と `src/interpreter/mod.rs` の全 6 箇所を統一。
- **エスケープシーケンス**: `parse_escape_sequence` 共通関数を抽出。`parse_char_literal` と `parse_string_literal` の両方から共通関数を呼び出すように変更。統一により `\"` が文字リテラル内でもサポートされるようになった。
- **WASM パイプライン**: 既に `analyze_source` / `analyze_and_optimize` に共通化済み（対応不要）。
- **Store/Retrieve**: 既に `_impl` + `emit_retrieve: bool` パラメータで統合済み（対応不要）。

### 未実施

- **constexpr 評価ロジック**: `semantic_analyzer` と `constexpr_eval` のコア式評価の統一は、3 テーブル方式と ConstexprEnv 方式の構造差異が大きく、リファクタリングリスクが高いため見送り。
