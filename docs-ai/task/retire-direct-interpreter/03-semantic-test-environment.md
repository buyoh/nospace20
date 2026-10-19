# 03: ツリー走査インタプリタを意味解析テスト環境 `semantic_test` に作り直す

作成日: 2026-10-19
ステータス: 設計完了（未実装）

## 設計・方針

### 位置づけ

- 用途: 「意味解析（と最適化）が出力する `Scope` が、言語仕様どおりの意味を持つか」を検証する正解器。large テストの `trace_hit_counts`・stdout・実行時エラーの検証と、`optimizer/tests.rs` の最適化前後の比較に使う
- 製品ビルド（CLI / WASM / ライブラリ）には含めない。`lib.rs` で `#[cfg(test)] mod semantic_test;` と宣言する
- 実装方針: 正しさと単純さを最優先する。性能・中断再開・ホスト連携（実 I/O）は持たない
- Cargo の分類上は unit test だが、テストの内容は「token_parser → … → semantic_analyzer → optimizer → テスト用インタプリタ」を通す結合テストである（要件との対応は README を参照）

### ディレクトリ構成

```text
src/semantic_test/
  mod.rs              # モジュール概要、RunOptions / RunOutcome / run_main（interpreter::run_main の再エクスポート）
  interpreter/
    mod.rs            # interpret_global / run_main（← interpreter/mod.rs）
    exec.rs           # ← interpreter/exec.rs
    exec_tests.rs     # ← interpreter/exec_tests.rs（ヘルパを書き換える）
    environment.rs    # ← interpreter/environment.rs（簡略化）
    allocator.rs      # ← interpreter/allocator.rs
    allocator_tests.rs
    types.rs          # ← interpreter/types.rs
    error.rs          # ← base/error/interpret_error.rs（VM 専用・I/O のバリアントを除く）
    error_tests.rs    # ← base/error/interpret_error_tests.rs
  large/
    mod.rs            # include!(concat!(env!("OUT_DIR"), "/generated_semantic_tests.rs"))
    base.rs           # ← tests/code_test/interpreter_base.rs と error_base.rs の test_runtime_error_base
```

- 名前 `semantic_test`: 「意味解析の検証用」であり、製品のインタプリタではないことを名前で示す。`interpreter` という語は、サブモジュール名（評価器そのもの）としてだけ残す
- `run_main` は、`Environment` のフィールドに直接触れるため `interpreter/mod.rs` に置き、`semantic_test/mod.rs` から再エクスポートする（`Environment` のフィールドは `pub(super)` のままでよい）
- `src/interpreter/` は削除する。`algorithm/alloc_spec.rs` のコメントにある `interpreter::allocator` の参照も更新する

### 実行 API

`lib.rs` の `interpret*`（7 関数）を、次の 1 つに統一する（`pub(crate)`、`#[cfg(test)]`）。

```rust
/// テスト実行の設定
pub(crate) struct RunOptions {
    /// 標準入力の内容
    pub stdin: String,
    /// 未初期化変数をランダム値で埋める（初期値 0 依存のバグ検出用）
    pub randomize_uninit: bool,
    /// 式評価回数の上限（無限ループでテストが止まらないようにする）
    pub max_expression_count: usize, // Default: 100_000（現行の interpret_func_testing と同じ）
}

/// テスト実行の結果
pub(crate) struct RunOutcome {
    /// __trace のキーごとのヒット回数
    pub traced: BTreeMap<i64, i64>,
    /// 標準出力の内容（__puti / __putc。__clog は含めない）
    pub stdout: String,
    /// __main の戻り値（return: がなければ None）
    pub return_value: Option<i64>,
}

/// グローバル初期化と __main を実行する。実行時エラーは panic（"runtime error: ..."）
pub(crate) fn run_main(scope: &Scope, options: &RunOptions) -> RunOutcome;
```

| 現行 API | 置き換え先 |
|---|---|
| `interpret_func_testing(scope, "__main")` | `run_main(scope, &RunOptions::default()).traced` |
| `interpret_func_testing_randomize` | `randomize_uninit: true` |
| `interpret_func_with_io(scope, "__main", stdin)` | `stdin` を指定し、`traced` と `stdout` を使う |
| `interpret_with_env` / `interpret`（`optimizer/tests.rs`） | `return_value`、または下記の書き換え |
| `interpret_func(scope, 任意の関数名)` | 削除（任意の関数を直接実行する用途はない） |

- 現行の `interpret_func_testing` は `let _ = interpret_func(...)` で `FunctionNotFound` を無視している。`run_main` では `__main` がなければ panic する（テストの書き誤りを見逃さないため）
- 実行時エラーは、現行どおり panic で表す（`runtime_panic` による `panic!("runtime error: {}", e)`）。テスト専用コードのため、panic がそのままテストの失敗になり、`runtime_error` テストは `catch_unwind` でメッセージを検証できる。AGENTS.md のエラーハンドリング方針の例外として、モジュールの doc に明記する。allocator / environment の内部 API の `Result` 化（runtime-robustness 02）は、そのまま残す
- `optimizer/tests.rs` の `test_noop_pass_magic_number_initialized` は、実行後に `env.allocator.get(env.global_base_addr + slot)` でグローバル変数を直接読んでいる。`RunOutcome` にはメモリを持たせず、このテストはソース側で `__trace(x)` や `return: x` を使って値を観測する形に書き換える（`RunOutcome` を小さく保つため）

### テストに不要な機能の削除・簡略化

| 機能 | 扱い | 理由 |
|---|---|---|
| `EnvironmentConfig::ignore_debug` と `exec.rs` の分岐 | 削除 | CLI / WASM ではコンパイル時に debug 拡張を外す方法に移った（01）。テスト環境では常に `__assert` / `__trace` が有効 |
| `__clog` の `println!`（ホストの stdout に出力） | 値を評価して返すだけにする（何も出力しない） | ホストの I/O を使わない。`__clog` の出力は製品経路（WS の `-13` と clog writer。[ws-vm-run-mode-support 02](../ws-vm-run-mode-support/02-clog-debug-ext.md)）の責務。success_io の期待値（`debug_clog_001` / `debug_return_001`）は従来も `__clog` を含まない。WS 側も clog をプログラムの stdout に混ぜないので、両方の経路で期待値は変わらない |
| `Environment::new()`（実 stdin/stdout）、`new_with_buffers`、`new_with_config` の `Box<dyn BufRead/Write>` | `Environment::new(stdin: &str, config)` 1 つにする。stdin は `Cursor<Vec<u8>>`、stdout は `Vec<u8>` を直接持つ | ホストの I/O を使わない。`SharedWriter`（`Rc<RefCell<_>>`）経由で stdout を共有する必要がなくなる |
| `EnvironmentMetrics`、`metrics()` | 削除 | 上限判定に使う `expression_count` は `Environment` の非公開フィールドにする |
| `max_expression_count: Option<usize>` | `usize`（必須） | テストで上限なしにする用途はない |
| `EnvironmentConfig::max_vm_frames` / `DEFAULT_MAX_VM_FRAMES` / `InterpretError::StackOverflow` | 削除（02） | VM 専用 |
| `InterpretError::Io` | 削除 | stdout が `Vec<u8>` なので書き込みは失敗しない。stdin も `Cursor` なので読み取りは失敗しない |
| `randomize_uninit` | 維持 | `_randomize` ターゲットで使う |
| allocator の解放済み・範囲外アクセス、不正 free、サイズ検証 | 維持 | `runtime_error` の alloc 系テスト 5 件（`alloc_freed_access_001`、`alloc_out_of_range_001`、`alloc_free_invalid_ptr_001`、`alloc_invalid_size_001`、`alloc_huge_size_001`）が依存する |

`exec.rs` の評価ロジック（文・式・スコープ・static 初期化）は変更しない。目的は移設と周辺の簡略化であり、意味論は変えない。

`exec_tests.rs` は、`Environment::new_with_config(Box<..>, Box<..>, EnvironmentConfig::new())` と `SharedWriter` を使うヘルパを持っている。新しい `Environment::new(stdin, config)` に合わせてヘルパを書き換える（テストの中身は変えない）。

### large テストの生成を分割する

ライブラリ内の `#[cfg(test)]` モジュールは、結合テストのクレート（`tests/*.rs`）から見えない。そこで、interpreter 系の生成テストをライブラリ側に移す。

| 生成ファイル | include 先 | 内容 |
|---|---|---|
| `generated_tests.rs`（既存） | `tests/code_test.rs` | `_ws`、`_ws_self*`、`syntax_error`、`compile_error`、`runtime_error` の `_ws_self`（新規、下記） |
| `generated_semantic_tests.rs`（新規） | `src/semantic_test/large/mod.rs` | success / success_io の素のテスト名（現行の interpreter ターゲット）、`_opt_all`、`_randomize`、`runtime_error` の素のテスト名 |

- `src_build/nospace_tests.rs` に、2 つのファイルへの writer を渡す（または生成関数を 2 つに分ける）。テスト名・パスは現行と同じにし、`cargo test <name>` の絞り込みが引き続き使えるようにする（モジュールパスは `semantic_test::large::` に変わる）
- `write_error_test` は現在、syntax_error / compile_error も `flags.has_interpreter` を条件に生成している。これらはインタプリタを使わないので、無条件に生成するように変える。`semantic-test` の除外は、semantic 側のテストだけに効くようにする
- `tests/code_test/interpreter_base.rs` は `src/semantic_test/large/base.rs` に移し、`run_main` を使うように書き換える。`error_base.rs` の `test_runtime_error_base` も同じファイルに移す（`test_syntax_error_base` / `test_compile_error_base` は `tests/` に残す）
- check.json の読み込み（`tests/code_test/test_config.rs`）は両方から使う。複製はせず、`large/base.rs` から `#[path = "../../../tests/code_test/test_config.rs"] mod test_config;` で同じファイルを読み込む（`serde` / `serde_json` は通常の dependencies なので使える）。lib 側で使わない variant / フィールドがあると dead_code 警告が出るため、その `mod` 宣言に `#[allow(dead_code)]` を付ける（`tests/code_test.rs` 側と同じ扱い）
- テストの実行時のカレントディレクトリは、ユニットテスト・結合テストともにクレートのルートなので、`resources/tests/...` の相対パスはそのまま使える

### 製品経路（compile → WhitespaceVM）の検証強化

正解器を製品から外す代わりに、製品の実行経路でも同じ期待値を検証する。

| 追加 | 内容 |
|---|---|
| `_ws_self`（debug 拡張あり）で `trace_hit_counts` を照合 | 現行の `test_whitespace_self_base_impl` は完走しか見ていない。`vm.traced` を check.json の `trace_hit_counts` と照合する（`has_debug_ext` が true の場合だけ。`_ws_self_strict` / `_ws_self_randomize` / `_ws_self_opt_all` も同様） |
| `runtime_error` の `_ws_self` | debug・alloc 拡張ありでコンパイルして WhitespaceVM で実行し、`StepResult::Error(_)` になることを確認する。`contains` は WS のメッセージと異なるので照合しない（エラーになることだけを確認） |

- WS 側が検出しないもの（alloc の解放済みアクセスなど）は、manifest で `exclude_targets: [whitespace-self]` を付ける。どれが該当するかは実装時に実行して確定し、該当テストの `comment` に理由を書く
- trace 照合で既存のテストが落ちた場合は、AGENTS.md に従ってテストを直さず Fail のまま残すか、`exclude_targets` で外す。どちらの場合も、原因と件数を本タスクの進捗に記録し、別タスクを起票する（意味解析とコンパイラの不一致を見つけること自体が、この強化の目的のため）

### `exclude_targets` の改名と検証

| 現行 | 変更後 | 使用箇所 |
|---|---|---|
| `interpreter` | `semantic-test` | manifest で 1 件（約 279 行目）と、9 行目付近のコメント |
| `interpreter-randomize` | `semantic-test-randomize` | 0 件 |
| `interpreter-opt-all` | `semantic-test-opt-all` | 0 件 |
| `nospace-vm` | 削除 | 0 件 |

- `TargetFlags` のフィールドも `has_semantic_test*` に改名する
- 現行の `TargetFlags::from_test_case` は、未知のターゲット名を黙って無視する。改名で古い名前が残っても気づけないため、既知の名前の一覧と照合し、未知の名前があれば build.rs で panic する（`whitespace`、`whitespace-self`、`whitespace-self-strict`、`whitespace-self-randomize`、`whitespace-self-opt-all`、`semantic-test`、`semantic-test-randomize`、`semantic-test-opt-all`）

### ライブラリ内の利用箇所と警告

- `src/optimizer/tests.rs`: `crate::interpret_func_testing` / `crate::interpret_with_env` / `crate::Environment` を `crate::semantic_test::run_main` に置き換える（`test_noop_pass_magic_number_initialized` は上記のとおり書き換える）
- `src/lib.rs`: `mod interpreter;` を削除し、`#[cfg(test)] mod semantic_test;` を追加する。`base::shared_writer` は `whitespace` / `wasm_api` が引き続き使うので残す
- `src/base/error/`: `NospaceError::Interpret`、`From<InterpretError>`、`CompileStage::NospaceExecution`、`pub use interpret_error::InterpretError` と、`base/error/tests.rs` の該当テストを削除する
- インタプリタだけが使っていた `pub(crate)` の関数・フィールドは、非テストビルドで dead_code 警告になる（例: `Scope::get_function`、`semantic_analyzer/scope.rs`）。`cargo build --release` で警告を洗い出し、テスト専用なら `#[cfg(test)]` を付け、どこからも使われないなら削除する
- そのほかの参照は `rtk proxy grep -rn "interpreter::\|interpret_\|EnvironmentConfig\|InterpretError" src tests examples` で洗い出す

### 移行の順序

1. `src/semantic_test/` を作り、`interpreter/` を移して `run_main` を実装する
2. build.rs の生成を分け、interpreter 系の large テストを `semantic_test::large` に移す。`tests/code_test/interpreter_base.rs` を削除する
3. `optimizer/tests.rs` を移行し、`lib.rs` の `interpret*` と再エクスポート、`NospaceError::Interpret`、`CompileStage::NospaceExecution` を削除する
4. 不要な機能（`ignore_debug`、`__clog` の出力、`EnvironmentMetrics` など）を削除する
5. `exclude_targets` を改名し、検証を追加する
6. 製品経路の検証を強化する

1〜3 は 1 つのコミットにまとめる。`cfg(test)` のモジュールに移した時点で、公開 API の `interpret*` も結合テストの `interpreter_base.rs` もそれを呼べなくなるため、途中の状態ではビルドが通らない。4 以降は、それぞれ `cargo test` が通る単位でコミットする。

## テスト

- 移行の前に `cargo test -- --list` で、interpreter 系（素の名前、`_opt_all`、`_randomize`、`runtime_error`）のテスト数を記録する。移行後に `semantic_test::large::` 配下の件数と一致することを確認する（`_vm` を除く）
- `exec_tests.rs`・`allocator_tests.rs`・`error_tests.rs` が、ヘルパの書き換え後に通ること
- `cargo build --release` と `cargo build --features wasm --target wasm32-unknown-unknown` で、dead_code などの新しい警告が出ないこと

## チェックリスト

1. [ ] `src/semantic_test/` への移設、`run_main`、生成の分割、`optimizer/tests.rs` の移行、公開 API の削除（移行の順序 1〜3）
2. [ ] 不要な機能の削除と `exec_tests.rs` のヘルパの書き換え（移行の順序 4）
3. [ ] `exclude_targets` の改名と、未知の名前の検出（移行の順序 5）
4. [ ] 製品経路の検証強化（移行の順序 6）
5. [ ] テスト件数の照合と、警告の確認
