# 直接実行インタプリタの廃止と意味解析テスト環境への作り直し

nospace のソースを直接実行するインタプリタを製品から外す。外すのはツリー走査インタプリタと NospaceVM。CLI の `--mode run` は「nospace20 コンパイル（独自拡張あり）→ whitespace20 VM で実行」に置き換える。ツリー走査インタプリタは、意味解析の結果を検証するテスト専用の環境（`#[cfg(test)]`）として作り直す。

作成日: 2026-10-19
ステータス: 設計完了（未実装）

前提タスク: [ws-vm-run-mode-support](../ws-vm-run-mode-support/README.md)（WhitespaceVM のメモリ上限、`__clog` の debug 拡張 API、`WasmWhitespaceVM` の修正）。本タスクは、その完了を前提とする。VM 自体の機能追加は本タスクでは行わない

## 背景・動機

- 直接実行インタプリタには、コンパイラやパーサと関係のない独自の問題が多い。コードレビュー 2026-10 では、05（実行時エラーの panic）、06（スコープ領域リーク）、07（再帰深度の上限なし）、10（`__geti` の三者不一致）がこれに当たる。2 系統（ツリー走査 `interpreter/exec.rs` と NospaceVM `interpreter/vm/`）あるため、修正も二重になる
- 利用者は CLI の `--mode run` と WASM の `WasmNospaceVM` だけで、実運用ではほぼ使われていない
- 一方、ツリー走査インタプリタは large テストで `__trace` のヒット回数（`trace_hit_counts`）を検証する唯一の経路である。WS 系の success テストは「完走すること」しか確認していない。また `runtime_error` テスト（assert 失敗、0 除算、alloc の不正アクセス検出）も、このインタプリタと NospaceVM だけで実行している。このため完全には削除せず、意味解析（と最適化）の結果を確かめるテストの「正解器」として残す。あわせて、製品の実行経路（compile → WhitespaceVM）でも trace と実行時エラーを検証するように large テストを強化する

## 設計・方針（要約）

| 項目 | 方針 | 詳細 |
|---|---|---|
| CLI `--mode run` | `compile_to_ws` → `WhitespaceVM` で実行する。debug 拡張は `--ignore-debug` がない限り常に有効、alloc 拡張は既定のまま。`main returns:` / `main exited` の出力は廃止。前提タスクで追加したメモリ上限（既定値）と `__clog` の出力先（stderr）を設定する | [01-cli-run-via-whitespace.md](01-cli-run-via-whitespace.md) |
| NospaceVM・WASM | `src/interpreter/vm/` と `WasmNospaceVM` を削除する。WASM で nospace を実行するには、`WasmWhitespaceVM::new`（nospace ソースを受け取りコンパイルする。前提タスクで debug 拡張の反映・`opt_passes`・メモリ上限に対応済み）を使う。移行表を用意する | [02-remove-nospace-vm-and-public-api.md](02-remove-nospace-vm-and-public-api.md) |
| ライブラリ公開 API | `interpret*` 関数、`Environment`、`EnvironmentConfig`、`InterpretError`、`NospaceVM`、`StepResult`（ルートの re-export）、`NospaceError::Interpret`、`CompileStage::NospaceExecution` を削除する | 同上 |
| ツリー走査インタプリタ | `src/interpreter/` を `src/semantic_test/`（`#[cfg(test)]`）に移す。テストに不要な機能（`ignore_debug`、実 stdin/stdout、メトリクス、VM 用の設定、任意関数の実行など）は削除する | [03-semantic-test-environment.md](03-semantic-test-environment.md) |
| large テスト | interpreter 系のターゲット（success / success_io / runtime_error の interpreter・opt_all・randomize）を、ライブラリ内の生成テスト `semantic_test::large` に移す。`_vm` ターゲットは削除する。`exclude_targets` 名を `semantic-test*` に改名する。製品経路の `_ws_self` で `trace_hit_counts` を照合し、`runtime_error` も検証する | 同上 |
| ドキュメント | README / docs / docs-ai / VERSION.md | [04-docs-and-release-notes.md](04-docs-and-release-notes.md) |

### 要件「結合テストとして作り直す」の実現方法

要件では「意味解析用の簡易テスト環境・結合テスト」とされている。本設計では、テスト環境の置き場所を `tests/`（Cargo の結合テストクレート）ではなく、ライブラリ内の `#[cfg(test)]` モジュールにする。理由は次のとおり（詳細は下の「採用しなかった案」）。

- インタプリタは `Scope` の `pub(crate)` な内部を読む必要がある。これは `tests/` からは参照できない
- 製品ビルドには含めない必要がある

テストの性質は「token_parser → tree_parser → semantic_analyzer → optimizer → テスト用インタプリタ」を通す結合テストであり、要件の意図は満たす。Cargo の分類上は unit test として実行される点だけが、要件の文言と異なる。

### 採用しなかった案

| 案 | 不採用の理由 |
|---|---|
| ツリー走査ではなく NospaceVM をテスト環境として残す | NospaceVM の利点は中断・再開（WASM のステップ実行）だが、テスト用途では使わない。ツリー走査のほうが実装が単純で、正解器に向いている（`interpreter/mod.rs` 冒頭の「極力シンプルな実装」という方針どおり） |
| テスト環境を cargo feature（例: `test-interpreter`）で切り替える | 自クレートを dev-dependency にして feature を有効化する方法は使えない（循環依存）。feature を手動指定しないと `cargo test` で large テストが走らなくなる |
| テスト環境を `tests/` 配下（結合テストのクレート）に置く | `Scope` の内部（`functions`、`variable_count` など）が `pub(crate)` であり、外から参照するには意味解析の内部構造を公開 API にする必要がある |
| `#[doc(hidden)] pub mod` として製品ビルドに残す | 「製品からは外す」という目的に反する。WASM のサイズも減らない |

## 他タスクとの関係

[runtime-robustness-2026-10](../../done-task/runtime-robustness-2026-10/README.md)（完了、コミット 84616dd）で実装した内容の一部を、本タスクで削除・置き換える。

| runtime-robustness の項目 | 本タスク後の扱い |
|---|---|
| 01: WASM 最適化パスの統一 | 影響なし |
| 02: NospaceVM 実行時エラーの Result 化 | VM 部分は VM ごと削除する。`InterpreterAllocator` / `Environment` の Result 化は、`semantic_test` 側にそのまま残す（ツリー走査側は `runtime_panic` で panic に変換する） |
| 03-A: パーサのネスト深度上限 | 影響なし（コンパイラ・意味解析のスタック保護に必要） |
| 03-C: CLI run の NospaceVM 移行 | 本タスク 01 で置き換える（`run_with_vm` を削除する） |
| 03-E: NospaceVM のフレーム数上限 | VM ごと削除する（`StackOverflow`、`max_vm_frames`、`DEFAULT_MAX_VM_FRAMES`）。無限再帰の保護は、前提タスクの WhitespaceVM のメモリ上限で引き継ぐ |

[runtime-robustness-followups.md](../runtime-robustness-followups.md) の項目は次のように扱われる。

- 1（インタプリタのヒープ総量の上限）: 製品の実行経路からインタプリタが外れ、前提タスクのメモリ上限に置き換わるので不要になる（`semantic_test` は `max_expression_count` で止まる）
- 5（`exec.rs` を廃止して NospaceVM に寄せる案）: 本タスクでは逆に、NospaceVM を廃止して `exec.rs` をテスト専用として残す。本タスクの方針で置き換える
- 6（`WasmWhitespaceVM::new` の最適化パス）: 前提タスクで解消する
- 7 のうち NospaceVM でのフィールド代入: NospaceVM の削除で不要になる（意味解析での左辺値の検査は残る）

着手の条件: ws-vm-run-mode-support が完了していること。

コードレビュー 2026-10 の指摘は次のように扱われる。

- 05・07 のインタプリタ部分: 製品コードから外れるため解消扱い
- 10（`__geti` の不一致）: 比較対象が「テスト環境と WS」の二者になるが、正解器との不一致としては残る

## 影響（互換性）

| 利用者 | 影響 | 対応 |
|---|---|---|
| CLI `--mode run` | stdout から `main returns: N` / `main exited` が消える。実行時エラーのメッセージが WS VM のものになる。alloc の解放済みアクセスなど、インタプリタだけが検出していたエラーは検出されなくなる。そのほかの差は [01](01-cli-run-via-whitespace.md) の差分表を参照 | README・VERSION.md に記載 |
| CLI フラグ | `--debug`、`--ignore-debug`、`--std-ext`、`--no-std-ext`、`--opt` は残す（webui のサーバ実行が `--debug` / `--ignore-debug` / `--std-ext` を渡している） | 意味は 01 を参照 |
| WASM `WasmNospaceVM` | 削除（破壊的変更）。外部プロジェクト [nospace20-webui](https://github.com/buyoh/nospace20-webui) の「直接実行」モード（`WasmExecutionBackend.ts` の `new nospace20.WasmNospaceVM(...)`）が動かなくなる | webui 側で `WasmWhitespaceVM` に移行する必要がある。移行表は [02](02-remove-nospace-vm-and-public-api.md)。別リポジトリのため本タスクの範囲外で、リリース前に調整する |
| ライブラリ API（`interpret*` など） | 削除（破壊的変更） | VERSION.md に記載 |

## タスクステップ・チェックリスト

0. [ ] 前提: [ws-vm-run-mode-support](../ws-vm-run-mode-support/README.md) の完了を確認する
1. [ ] 01: CLI `--mode run` を compile + WhitespaceVM に置き換える
   1. [ ] `CompileProperty::run_compile_options` と `bin/nospace20.rs` の Run 分岐（`run_with_vm` を置き換える）
   2. [ ] `ignore_debug_test.rs` を WS ベースのテストに書き換える
2. [ ] 02: NospaceVM と `WasmNospaceVM` を削除する
   1. [ ] `src/interpreter/vm/`、`wasm_api/nospace_vm.rs`、`tests/code_test/nospace_vm_base.rs` を削除し、`_vm` ターゲットの生成もやめる
   2. [ ] `tools/wasm-test/test.mjs` の移行
3. [ ] 03: `src/semantic_test/` に作り直す
   1. [ ] `src/interpreter/` を `src/semantic_test/interpreter/` に移して `run_main` を実装し、large テストの生成を分割し、`optimizer/tests.rs` を移行し、`lib.rs` の `interpret*` と再エクスポートと `NospaceError::Interpret` を削除する（1 コミット）
   2. [ ] テストに不要な機能を削除する
   3. [ ] `exclude_targets` を改名し、未知のターゲット名を build で検出する
   4. [ ] 製品経路の検証を強化する（`_ws_self` の trace 照合、`runtime_error` の `_ws_self`）
4. [ ] 04: ドキュメント・VERSION.md
5. [ ] `cargo test`、`cargo build --release`（警告なし）、`cargo build --features wasm --target wasm32-unknown-unknown` と `tools/wasm-test`、`cargo clippy` を確認し、done-task へ移動する

ステップ 1 → 2 → 3 の順に実施する。1 で CLI は NospaceVM を使わなくなり、2 で VM と WASM の利用箇所がなくなる。各ステップの終わりで `cargo test` が通る状態を保つ。
