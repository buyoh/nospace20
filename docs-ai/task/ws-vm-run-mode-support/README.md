# WhitespaceVM の機能追加（nospace 実行経路の前提整備）

[retire-direct-interpreter](../retire-direct-interpreter/README.md) で CLI `--mode run` と WASM の直接実行を「compile → WhitespaceVM」に置き換える前に、WhitespaceVM 側に必要な機能を追加する。本タスクは VM（と WASM ラッパ、`whitespace20`）の機能追加だけを行う。NospaceVM の置き換え（CLI run の書き換え、`WasmNospaceVM` の削除）は行わない。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 背景・動機

NospaceVM には、WhitespaceVM にない次の機能・保護がある。置き換えの時点でこれらが欠けていると、利用者から見て退行になる。

| NospaceVM（現行の CLI run / `WasmNospaceVM`） | WhitespaceVM の現状 | 本タスク |
|---|---|---|
| フレーム数上限（`max_vm_frames`）で、無限再帰を `StackOverflow` エラーにする | 上限がない。無限再帰ではホストのメモリが尽きるまで伸び続ける（CLI は OOM、WASM はタブが落ちる） | [01](01-memory-limit.md): VM が使うメモリの上限 |
| `__clog(n)` で `__clog: n` を出力する | コンパイラが `__clog` を noop にしているため、何も出ない | [02](02-clog-debug-ext.md): debug 拡張 API `-13` |
| `WasmNospaceVM(source, stdin, opt_passes, ignore_debug)` で、debug 拡張・最適化付きで実行する | `WasmWhitespaceVM::new` は debug 拡張を VM に反映しない（バグ）。最適化パスを受け取らない。エラー文字列が `Debug` 形式 | [03](03-wasm-whitespace-vm.md): WASM ラッパの修正 |

## 設計・方針（要約）

| 項目 | 方針 |
|---|---|
| メモリ上限 | Whitespace にはコールスタックという概念がない（`Call` / `Return` の戻り先は処理系の内部状態）。nospace の関数呼び出しのフレームはヒープとデータスタックに置かれる。そのため「呼び出しの深さ」ではなく、**VM が使うメモリの量**（ヒープのセル数 + データスタックの要素数 + 戻り先の数）に上限を設け、実質的にコールスタックを制限する。名前に `call` は使わない（`max_memory_cells`、`--max-memory-cells`） |
| `__clog` | debug 拡張 API にアドレス `-13` を追加する。出力先は VM の専用 writer（プログラムの stdout とは別）。debug 拡張が無効なら、`-13` への store は普通のメモリ書き込みであり、何もトリガーしない |
| WASM | `WasmWhitespaceVM::new` で debug 拡張を VM に反映する（バグ修正）。`opt_passes` とメモリ上限を受け取る。エラー文字列を `Display` 形式にする |

## 他タスクとの関係

- [retire-direct-interpreter](../retire-direct-interpreter/README.md): 本タスクの完了を前提に、CLI run と WASM の直接実行を置き換える
- [ws-vm-input-via-heap-store.md](../ws-vm-input-via-heap-store.md): 入力命令のヒープ書き込みを `heap_store` に一元化する。01 のメモリ上限の確認と 02 の `-13` フックは、`heap_store` に置く。入力命令の書き込みも同じ扱いにするため、**ws-vm-input-via-heap-store を先に実施する**。先に実施できない場合は、入力命令の書き込み箇所にも上限の確認を入れ、その旨を記録する
- [runtime-robustness-followups.md](../runtime-robustness-followups.md) の 6（`WasmWhitespaceVM::new` が最適化パスを受け取らない）: 03 で解消する
- [cli-errors-and-doccomments](../cli-errors-and-doccomments/): `whitespace20` のエラー表示を `Display` にする。01 で追加するエラーのバリアントは、`Display` を実装すれば、そちらの変更に自然に含まれる

## タスクステップ・チェックリスト

1. [ ] 01: メモリ上限（`WhitespaceVM::with_max_memory_cells`、`WsRuntimeError::MemoryLimitExceeded`、`whitespace20 --max-memory-cells`）
2. [ ] 02: `__clog` の debug 拡張 API `-13`（コンパイラ、VM、`whitespace20`）
3. [ ] 03: `WasmWhitespaceVM` の修正（debug 拡張の反映、`opt_passes`、メモリ上限、`Display` のエラー文字列）と `tools/wasm-test`
4. [ ] `docs/spec-whitespace.md`（`-13`、メモリ上限）、`docs/spec.md`（`__clog` の出力先）、VERSION.md
5. [ ] `cargo test`、`cargo build --features wasm --target wasm32-unknown-unknown` と `tools/wasm-test` を確認し、done-task へ移動する
