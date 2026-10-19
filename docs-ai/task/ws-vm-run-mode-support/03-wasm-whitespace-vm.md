# 03: `WasmWhitespaceVM` の修正（nospace の直接実行に使える状態にする）

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 背景・動機

`WasmWhitespaceVM::new(nospace_source, stdin, interactive?, std_extensions?)` は、nospace ソースを解析・コンパイルして VM を作れる。retire-direct-interpreter で `WasmNospaceVM` を削除したとき、代わりに使えるようにするため、次の問題を直す。`WasmNospaceVM` の削除そのものは本タスクでは行わない。

## 設計・方針

### バグ修正: debug 拡張が VM に反映されない

- `create_from_ws_source` が `vm.with_debug_ext(false)` を固定で呼んでいる（`wasm_api/whitespace_vm.rs`）。そのため `std_extensions: ["debug"]` を渡すと、コンパイラは debug 拡張付きのコードを出力するが、VM は `__trace` を記録せず、`__assert` も失敗しない（負アドレスへの普通のヒープ書き込みになる）
- 修正: `create_from_ws_source` に `debug_ext: bool` を引数として追加する。`new()` は `parse_std_extensions` の結果を渡す。`fromWhitespace` / `fromWhitespaceInteractive`（WS ソースから直接作る API）は引数を増やさず、`false` を渡す（現行の挙動を維持）
- AGENTS.md に従い、修正箇所にバグの原因と解決策をコメントで残す

### 追加する引数

`WasmWhitespaceVM::new` の末尾に、省略可能な引数を追加する。JS では末尾の省略可能引数の追加なので、既存の呼び出しは壊れない。

| 引数 | 内容 |
|---|---|
| `opt_passes: Option<JsOptPassArray>` | `pipeline::analyze_source` を `pipeline::analyze_and_optimize` に置き換え、得た `OptimizationOptions` を `WsCompileOptions.optimization` に渡す（peephole も適用される） |
| `max_memory_cells: Option<u32>` | [01](01-memory-limit.md) の上限。省略時は既定値（`DEFAULT_MAX_MEMORY_CELLS`、または 01 で検討する WASM 用の値）。`0` は無制限（この層で `None` に変換する）。ブラウザのタブを落とさないため、既定を有限にする。既存の `new()` 利用者にも上限が掛かる挙動の変更になる（`fromWhitespace*` は無制限のまま） |

### エラー文字列

- `step()` のエラーは現在 `format!("{:?}", e)`（例: `DivisionByZero`）である。これを `format!("{}", e)`（例: `division by zero`）に変える。`WsRuntimeError` の `Display` は実装済み
- 文字列が変わるので、VERSION.md に記載する（下記）

### `__clog`

- WASM では clog writer を設定しない（出力を捨てる）。`WasmNospaceVM` の `println!` も WASM では捨てられていたので、挙動は同じ。取得 API（例: `flushClog`）は、需要が出たら別タスクで追加する

## VERSION.md（WIP）に追加する項目（案）

- WASM `WasmWhitespaceVM::new`: The debug extension passed in `std_extensions` is now applied to the VM (previously `__trace` was not recorded and `__assert` never failed). Programs that pass `["debug"]` may now stop with an assertion error
- WASM `WasmWhitespaceVM::new`: Accept `opt_passes` and `max_memory_cells`. A memory limit is now applied by default (`0` for unlimited)
- **Breaking** WASM `WasmWhitespaceVM`: Runtime errors from `step()` use the display message (e.g. `division by zero` instead of `DivisionByZero`)
- whitespace20 / Whitespace VM: Add an optional memory limit (`--max-memory-cells`, unlimited by default) and the debug extension API `-13` for `__clog` (written to stderr)

## テスト

`tools/wasm-test/test.mjs` に、次のケースを追加する（既存の `WasmNospaceVM` のケースは、retire-direct-interpreter で移行するので、ここでは触らない）。

- `std_extensions: ["debug"]` で `__trace` が `get_traced()` に記録され、`__assert(0)` が `error` になる（バグ修正の確認）
- `std_extensions` なしでは `__trace` が記録されない
- `opt_passes: ["all"]` を渡しても同じ出力になる
- 0 除算のエラー文字列が `division by zero`
- `max_memory_cells` を小さく設定すると、末尾でない無限再帰（`return: f() + 1;`）が `memory limit exceeded` で止まる

## チェックリスト

1. [ ] debug 拡張の反映（バグ修正）
2. [ ] `opt_passes` と `max_memory_cells` の引数
3. [ ] エラー文字列の `Display` 化
4. [ ] `tools/wasm-test/test.mjs` のケース追加
