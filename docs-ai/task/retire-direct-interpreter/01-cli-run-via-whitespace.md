# 01: CLI `--mode run` を compile + WhitespaceVM に置き換える

作成日: 2026-10-19
ステータス: 設計完了（未実装）

前提: [ws-vm-run-mode-support](../ws-vm-run-mode-support/README.md) が完了していること。このタスクでは、そこで追加した VM の機能（`with_max_memory_cells`、`DEFAULT_MAX_MEMORY_CELLS`、`with_clog_writer`、`__clog` の `-13`）を使うだけで、VM 自体には手を入れない。

## 設計・方針

### 前提（現在のコード）

`bin/nospace20.rs` の Run 分岐は、runtime-robustness 03-C（コミット 84616dd）により `run_with_vm`（`NospaceVM` / `StepResult` / `EnvironmentConfig` / `InterpretError` を使う）になっている。これを WS の経路に置き換える。

### 実行パイプライン

`--mode run` でも `--mode compile` と同じ解析・最適化を行う（現行と同じ）。その後の処理を次のように変える。

```text
parse_to_tokens → parse_to_tree → semantic_analyze → optimize(--opt)
  → compile_to_ws(property.run_compile_options(opt_options))
  → WhitespaceVM::from_source(&ws)
       .with_debug_ext(debug_ext)
       .with_max_memory_cells(Some(DEFAULT_MAX_MEMORY_CELLS))
       .with_clog_writer(stderr)
       .with_io(BufReader(stdin), stdout)
  → vm.run(usize::MAX) → vm.flush()
```

- WS を文字列にしてから `from_source` で読み直す。テスト（`whitespace_self_base.rs`）や `ws_profiler` と同じ経路にするため、`WsProgram` を直接 VM に渡す API は追加しない
- `strict_heap` / `randomize_heap` は false（`whitespace20` の既定と同じ）
- メモリ上限を CLI のオプションで変えられるようにするかは、本タスクでは決めない（既定値だけを使う。必要になったら `--max-memory-cells` を `whitespace20` と同じ名前で追加する）

### 拡張の決め方

| 拡張 | `--mode run` | `--mode compile`（変更なし） |
|---|---|---|
| debug | **`--ignore-debug` がなければ常に有効**。`--std-ext debug` は指定してもしなくてもよい | `--std-ext debug` で有効 |
| alloc | `build_target_extensions()` に従う（既定で有効、`--no-std-ext` で無効） | 同左 |

- 理由: 現行の `--mode run` では、`__assert` / `__trace` / `__clog` が常に有効だった（`ignore_debug` の場合だけ無効）。run モードの実行先は自前の whitespace20 VM なので、拡張 API を使ってよい。このため挙動を保つ
- `--ignore-debug` は「debug 拡張なしでコンパイルする」と同じ意味にする。このときコンパイラは `__assert` などを `generate_builtin_debug_noop` で処理する（第 1 引数の値を残し、残りの引数は副作用のため評価して捨てる）。現行インタプリタの `ignore_debug` と同じく、副作用は保持される
- `--ignore-debug` と `--std-ext debug` を同時に指定した場合は、`--ignore-debug` を優先する
- 決定ロジックは `CompileProperty::run_compile_options(&self, optimization: OptimizationOptions) -> WsCompileOptions` として置く（`bin/nospace20.rs` の `main` に直接書かない）。CLI の引数解析から切り離して、ユニットテストを書けるようにするため。VM 側の `with_debug_ext` には、返した `WsCompileOptions.debug_ext` を使う

### 出力・終了コード

| VM の結果 | 現行（NospaceVM） | 変更後 |
|---|---|---|
| 完了 | stdout に `main returns: N` または `main exited` | **何も出力しない**。exit 0 |
| 実行時エラー | stderr に `error: {InterpretError}`、exit 1 | stderr に `error: {WsRuntimeError}`（`Display`）、exit 1 |
| `Suspended`（`usize::MAX` ステップでは実質到達しない） | — | stderr に `error: execution step limit exceeded`、exit 1 |
| `WaitingForInput`（非対話モードでは到達しない） | — | stderr に `error: unexpected input wait`、exit 1 |
| `--debug` | `=== Trace Results ===` と `trace[k]: v` を stdout に出力 | 同じ形式で `vm.traced` を出力する（`--ignore-debug` なら空なので出力しない） |

- `main returns:` を廃止する理由: WS にコンパイルしたプログラムでは `__main` の戻り値を取り出せない。またプログラム自身の出力に処理系のメッセージが混ざるため、`whitespace20` と同じく出力しない
- VM 終了後は、エラーの有無にかかわらず `vm.flush()` を呼ぶ（エラー前の出力を失わないため）
- `__main` が存在しない場合の検出（`a.has_function("__main")`）は、現行のまま残す
- `compile_to_ws` のエラーは、compile モードと同じく `error: {e}` で exit 1

### 実行時の挙動の違い（既知、仕様として受け入れる）

| 項目 | 現行（NospaceVM） | 変更後（WS VM） |
|---|---|---|
| alloc の解放済み・範囲外アクセス、不正 free、二重解放 | 検出してエラー | 検出しない（WS の alloc ランタイムの仕様）。どこまで検出するかは、03 の `runtime_error` `_ws_self` で確定する |
| 無限再帰・深い再帰 | フレーム数上限で `StackOverflow` | メモリ上限で `MemoryLimitExceeded`（ws-vm-run-mode-support 01） |
| `__alloc` の総量 | 上限なし（runtime-robustness-followups 1） | メモリ上限に含まれる |
| `--no-std-ext` で `__alloc` / `__free` を使う | 実行できる（インタプリタは常にアロケータを持つ） | コンパイルエラー（compile モードと同じ） |
| `__trace(n)` の戻り値 | 0 | `n`（`generate_builtin_debug_store` の仕様）。`docs/spec.md` は「戻り値は void」としているので、値を使うプログラムは仕様外。本タスクでは揃えない（`semantic_test` も 0 のまま） |
| `__clog(n)` の出力 | ホストの stdout に `__clog: n` | **stderr** に `__clog: n`。プログラムの stdout には混ざらない。VM は `__clog` の前に stdout を flush するので、端末上の順序は概ね保たれるが、別ストリームのため順序は保証しない |
| `__geti` の入力解釈 | NospaceVM の実装 | WS の実装（コードレビュー 10 で別途扱う） |
| 実行時エラーのメッセージ | `InterpretError` | `WsRuntimeError` |

### その他の変更

- `#[command(about)]` と `Args` の doc を「インタプリタ」から更新する（例: `A nospace language compiler (run mode executes the compiled Whitespace on the built-in VM)`）
- `compile_property.rs`: `ExecutionMode::Run` の doc「インタプリタモード」を「コンパイルして内蔵 Whitespace VM で実行」に更新する。`target_extensions` の doc「mode=Compile 時のみ使用」も、run で使うように直す
- [cli-errors-and-doccomments](../cli-errors-and-doccomments/) と同時期に実施する場合は、エラー出力の形式（stderr の統一など）をそちらの方針に合わせる

## テスト

- `CompileProperty::run_compile_options` のユニットテスト: `ignore_debug` の有無と、`--std-ext debug` / `--no-std-ext` の組み合わせ
- `tests/ignore_debug_test.rs` の書き換え: 現行の 8 件は、インタプリタの `EnvironmentConfig { ignore_debug }` と `should_panic` を使っている。これを「`debug_ext` を指定してコンパイル → `WhitespaceVM` で実行」に変える。`helpers.rs` は、compile + VM 実行（`StepResult`、stdout、clog の出力、`traced` を返す）のヘルパに置き換える

| 現行のテスト | 移行後 |
|---|---|
| `ignore_debug_assert_does_not_panic` | `debug_ext: false` で `__assert(0)` が完走し、stdout が `42` |
| `ignore_debug_assert_not_does_not_panic` | 同上（`__assert_not(1)`） |
| `ignore_debug_preserves_side_effects` | `debug_ext: false` で `__assert(a = a + 2)` の副作用が残る |
| `ignore_debug_trace_does_not_record` | `debug_ext: false` で `vm.traced` が空 |
| ignore 系の clog テスト | `debug_ext: false` で、clog writer にも stdout にも何も出ない |
| `should_panic` の assert 2 件（通常系） | `debug_ext: true` で `StepResult::Error(WsRuntimeError::AssertionFailed(_))` になる（`should_panic` をやめる） |
| 通常系の trace 記録 | `debug_ext: true` で `vm.traced` に記録される |

- CLI の手動確認: README の fibonacci の例、`--debug` の trace 表示、`--ignore-debug`、`__clog`（stderr に出る）、実行時エラー（0 除算、`__assert(0)`、無限再帰）の exit コード

## チェックリスト

1. [ ] `CompileProperty::run_compile_options` とユニットテスト
2. [ ] `bin/nospace20.rs` の `run_with_vm` を WS の経路に置き換え、`NospaceVM` / `EnvironmentConfig` / `InterpretError` の import を外す
3. [ ] `tests/ignore_debug_test.rs` を WS ベースに書き換える
4. [ ] CLI の手動確認
