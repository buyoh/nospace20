# 04: ドキュメント・VERSION.md の更新

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

| ファイル | 更新内容 |
|---|---|
| `README.md` | 冒頭（5・7 行目付近）の「interpret / interpreter として動作」という説明を「compiler（run モードは内蔵 Whitespace VM で実行）」に改める。「CLI Usage」の run の例から `main exited` の行を削除する。`--mode run` は「コンパイルして内蔵 Whitespace VM で実行する」と説明し、debug 拡張が既定で有効なこと・`--ignore-debug` の意味を書く。「Compile and run」の例はそのまま |
| `docs/README.md` | 3 行目の「compiler/interpreter」を改める |
| `docs/spec.md` §「nospace20 処理系の整数演算」（999 行目付近） | 対象処理系の列挙から「インタプリタ `nospace20`（`--mode run`）、NospaceVM」を削除し、「`nospace20 --mode run`（Whitespace へコンパイルして `whitespace20` と同じ VM で実行）」に改める。ツリー走査インタプリタはテスト用の正解器として、この節の挙動に従うことを注記する。処理系の上限（runtime-robustness で Appendix に追加した NospaceVM のフレーム数上限など）があれば削除し、前提タスクで記載した WhitespaceVM のメモリ上限を参照する |
| `docs/spec.md` の `__clog` / `__trace` の説明 | 出力先・戻り値の記述が、01 の差分表の挙動と合っているかを確認する（`-13` と `__clog` の出力先は前提タスクで記載済み） |
| `docs/optimize.md` | 冒頭の「コンパイラ・インタプリタ」を「コンパイラ」に改める |
| `docs-ai/architecture/README.md` | 処理フロー図の `Interpreter (interpreter) 直接実行` を削除し、`Compiler WS → WhitespaceVM` の 1 本道にする。`semantic_test`（テスト専用）を注記する |
| `docs-ai/architecture/overview.md`、`modules.md` | `interpreter/` の節を `semantic_test/`（`#[cfg(test)]`、意味解析の正解器）に置き換え、NospaceVM の記述を削除する |
| `docs-ai/spec/implementation-status.md` | 「interpreter」の行・節を「semantic_test（テスト専用）」に改める |
| `docs-ai/spec/compiler-rust-impl/`（`api-cli.md` の `interpret_func` など） | `rtk proxy grep -rn "interpret\|NospaceVM\|interpreter" docs-ai/spec docs-ai/architecture` で洗い出し、削除した API・モジュールへの言及を更新する |
| `docs-ai/spec/compiler-test-strategy.md` など、テストのターゲットを説明している文書 | `exclude_targets` の新しい名前、`_vm` ターゲットの廃止、`_ws_self` の trace 照合・`runtime_error` の追加 |
| `AGENTS.md` のディレクトリ構造 | `interpreter/` を `semantic_test/  # 意味解析の検証用インタプリタ（テスト専用）` に改める |
| `VERSION.md`（WIP） | 下記 |
| `docs-ai/task/README.md` | 完了時に done-task へ移す |

### VERSION.md（WIP）に追加する項目（案）

- CLI: `--mode run` now compiles to Whitespace (with the debug extension unless `--ignore-debug`, and alloc unless `--no-std-ext`) and runs it on the built-in Whitespace VM. The `main returns:` / `main exited` lines are no longer printed
- **Breaking** WASM: Remove `WasmNospaceVM`; use `WasmWhitespaceVM` (constructed from nospace source) instead. Pass `["debug", "alloc"]` as `std_extensions` to get the previous default behavior
- **Breaking** Library: Remove the direct interpreter API (`interpret*`, `Environment`, `EnvironmentConfig`, `InterpretError`, `NospaceVM`, `StepResult`, `NospaceError::Interpret`)

現在の WIP 見出しは `2.0.1 (WIP)`（patch バージョン）であり、破壊的変更を載せるのにふさわしくない。実装時に見出しを `3.0.0 (WIP)` に変えることを提案し、ユーザーの判断を仰ぐ（本タスクで勝手に変えない）。

## チェックリスト

1. [ ] 上表のドキュメント更新
2. [ ] VERSION.md と、WIP の見出しのバージョンの確認
