# 02: NospaceVM と `WasmNospaceVM` を削除する

作成日: 2026-10-19
ステータス: 設計完了（未実装）

前提: [ws-vm-run-mode-support](../ws-vm-run-mode-support/README.md) の 03 で、`WasmWhitespaceVM::new` が次の状態になっていること: debug 拡張が VM に反映される、`opt_passes` と `max_memory_cells` を受け取る、エラー文字列が `Display` 形式。

## 設計・方針

### 削除するもの（このステップ）

| 対象 | 備考 |
|---|---|
| `src/interpreter/vm/`（`mod.rs`、`eval.rs`、`exec.rs`、`scope.rs`、`tests.rs`） | NospaceVM 一式 |
| `src/wasm_api/nospace_vm.rs` と、`wasm_api/mod.rs` の `mod nospace_vm;` | `WasmNospaceVM` |
| `wasm_api/types.rs` のうち、NospaceVM だけが使う型 | `VmStepResult` / `JsVmStepResult` は `WasmWhitespaceVM` も使うので残す。実装時に参照箇所を確認する |
| `lib.rs`: `pub use interpreter::{NospaceVM, StepResult}` | `StepResult` はルートから消える（`whitespace::StepResult` は残る） |
| `EnvironmentConfig::max_vm_frames`、`DEFAULT_MAX_VM_FRAMES`、`InterpretError::StackOverflow` | runtime-robustness 03-E で追加した VM 専用のもの |
| `tests/code_test/nospace_vm_base.rs` と、`tests/code_test.rs` の `mod nospace_vm_base` | |
| `src_build/common.rs` の `TargetFlags::has_nospace_vm`、`src_build/nospace_tests.rs` の `_vm` テストの生成（success / success_io / runtime_error） | `exclude_targets: [nospace-vm]` の使用は現在 0 件 |

`interpret*` 関数、`Environment` / `EnvironmentConfig` / `InterpretError` の再エクスポート、`NospaceError::Interpret`、`CompileStage::NospaceExecution` は、ツリー走査インタプリタの移設と同時に削除する（[03](03-semantic-test-environment.md) の移行の順序 1〜3。`cfg(test)` に移した時点で、公開 API から呼べなくなるため）。

### `WasmNospaceVM` から `WasmWhitespaceVM` への移行表

webui などの利用者向け。VERSION.md と README の互換性欄から参照する。

| `WasmNospaceVM` | `WasmWhitespaceVM::new` での代替 |
|---|---|
| `new(source, stdin, opt_passes?, ignore_debug?)` | `new(source, stdin, interactive?, std_extensions?, opt_passes?, max_memory_cells?)` |
| `ignore_debug` 省略時（既定）は debug 有効 | `std_extensions` を省略すると、**debug も alloc も無効**になる。従来と同じにするには `["debug", "alloc"]`、`ignore_debug` 相当なら `["alloc"]` を明示して渡す |
| `step()` のエラー文字列（`InterpretError` の `Display`） | `WsRuntimeError` の `Display`（文言は異なる。例: 無限再帰は `memory limit exceeded: N cells`） |
| `flushStdout()` / `getTraced()` | `flush_stdout()` / `get_traced()`（snake_case。名前の統一は本タスクでは行わない） |
| `getReturnValue()` | なし（WS では取得できない） |
| `is_complete()` / `total_steps()` | 同名のメソッドがある。`total_steps` の単位は「式評価数」から「WS 命令数」に変わる |
| `__clog` の出力（WASM では捨てられていた） | 捨てられる（挙動は同じ） |

- `api.rs` 冒頭のコメント（`run()` は `WasmNospaceVM` で置き換えた、という記述）と、`wasm_api/mod.rs` のモジュール一覧の doc を更新する

### `tools/wasm-test/test.mjs`

- 「1. WasmNospaceVM tests」の各ケース（`__puti` 出力、`__trace`、複数行出力など）を `WasmWhitespaceVM` に移す。`__trace` / `__assert` を検証するケースは `std_extensions: ["debug"]` を渡す。`flushStdout` / `getTraced` は `flush_stdout` / `get_traced` に読み替える
- `return: 42` の戻り値を検証するケースは削除する
- 解析エラー（未定義変数・未定義関数・重複宣言・未定義変数への代入）のケースは、`WasmWhitespaceVM` のコンストラクタが同じ `ResultErr` を投げることを確認する形に移す。同じ内容のケースが ws-vm-run-mode-support 03 ですでに追加されていれば、重複させない

## テスト

- `cargo test`（`_vm` テストが消え、それ以外は変化しない）
- `cargo build --features wasm --target wasm32-unknown-unknown` と `tools/wasm-test`

## チェックリスト

1. [ ] NospaceVM、`WasmNospaceVM`、`nospace_vm_base.rs`、`_vm` テストの生成、VM 専用の設定とエラーを削除する
2. [ ] `tools/wasm-test/test.mjs` を移行する
