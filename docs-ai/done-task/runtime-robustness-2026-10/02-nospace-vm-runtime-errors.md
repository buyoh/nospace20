# 02: NospaceVM の実行時エラーを戻り値で返し、`__alloc` のサイズを検証する

作成日: 2026-10-19
ステータス: 完了

## 背景・動機

[05-runtime-error-panic.md](../code-review-2026-10/05-runtime-error-panic.md) 参照。採用案 C + B。

## 設計・方針

### InterpretError の追加バリアント

| バリアント | Display（`runtime error:` は付けない） | 発生元 |
|---|---|---|
| `DivisionByZero` | `division by zero` | `/` `%`（VM） |
| `AssertionFailed(Option<i64>)` | `assertion failed` / `assertion failed: {v} != 0` | `__assert` / `__assert_not` |
| `InvalidMemoryAccess(i64)` | `invalid memory access at address {a}` | allocator get/set |
| `FreedMemoryAccess(i64)` | `access to freed memory at address {a}` | allocator get/set |
| `InvalidFree(i64)` | `free: invalid address {a}` | free |
| `FreeNotBlockStart { addr, block_start }` | `free: address {a} is not a block start (block starts at {b})` | free |
| `DoubleFree(i64)` | `double free at address {a}` | free |
| `InvalidAllocSize(i64)` | `alloc: invalid size {n}` | alloc |
| `InvalidCharCode(i64)` | `__putc: invalid character code {c}` | `__putc` |
| `ExpressionLimitExceeded(usize)` | `expression evaluation limit exceeded: {n}` | `max_expression_count` |
| `StackOverflow(usize)` | `stack overflow: frame limit {n} exceeded` | VM フレーム数上限（[03](03-nesting-depth-and-cli-vm.md)） |
| `Io(String)` | `I/O error: {msg}` | stdout 書き込み・flush |

メッセージは既存の panic メッセージの部分文字列（runtime_error テストの `contains`）を保つ。

### API 変更

- `InterpreterAllocator::{alloc, free, get, set}` → `Result<_, InterpretError>`
- `Environment::{write_int, write_char, flush, increment_expression_count}` → `Result<(), InterpretError>`
- NospaceVM: 上記を `?` 相当で伝播し `ExecuteResult::Error` にする。`exec_builtin` / `exec_internal_builtin` / 変数の get/set ヘルパも Result 化。`NospaceVM::flush()` は `Result<(), InterpretError>` を返す
- `exec.rs`（ツリー走査インタプリタ）: 呼び出し側で `runtime_panic(result)`（`panic!("runtime error: {}", e)`）を使い、従来の panic 挙動とメッセージを維持
- `eval.rs` の `cannot take reference` は意味解析で排除される内部不整合のため panic のまま（AGENTS.md の「回復不能な異常」）。`left value is not assignable` は当初同じ扱いとしたが、レビューで意味解析を通過する入力があると判明したため `InterpretError::NotAssignable` に変更

### `__alloc` サイズ検証（05-C）

- `alloc(n)`: `n < 0` または `n > MAX_ALLOC_WORDS` で `InvalidAllocSize(n)`
- `MAX_ALLOC_WORDS = 1 << 24`（16M ワード = 128MiB）を `interpreter/allocator.rs` に定義（インタプリタ固有の上限。WS 側 `alloc_runtime` は Whitespace ヒープの性質上チェックしない）
- バンプポインタは `checked_add`、失敗時 `InvalidAllocSize`
- spec.md: 言語仕様に「n が負、または処理系の上限を超える場合の挙動は実装依存」、Appendix に「nospace20 のインタプリタでは実行時エラー（上限 16777216 ワード）」を追記

### テスト

- アロケータのユニットテスト: 既存の `#[should_panic]` テストを戻り値判定に変更（API 変更に伴う修正）、負・巨大サイズ・`next_addr` オーバーフロー
- VM のユニットテスト: 0 除算・assert・不正アクセス・二重 free・出力エラー（書き込み失敗する Writer のスタブ）・`max_expression_count` 超過が `StepResult::Error` になる
- `tests/code_test/nospace_vm_base.rs` の `test_runtime_error_base_vm` を `catch_unwind` から戻り値判定に変更し、`contains` もエラーメッセージで検証
- large テスト追加: `fails/runtime/div_zero_001`（存在しなければ）、`alloc_invalid_size_001`（`__alloc(-1)`）、`alloc_huge_size_001`

## チェックリスト

1. [x] `InterpretError` バリアント追加
2. [x] allocator / environment の Result 化、exec.rs の明示 panic 化
3. [x] NospaceVM の Result 伝播
4. [x] alloc サイズ検証
5. [x] テスト（unit / large / VM ハーネス）
6. [x] spec.md、VERSION.md
