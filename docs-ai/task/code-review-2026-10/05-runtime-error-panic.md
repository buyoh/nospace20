# インタプリタ: ユーザープログラムの実行時エラーを panic! で表現している

- 重要度: **high**
- 状態: 未着手

## 進捗

- [ ] NospaceVM（WASM 経路）の Result 化
- [ ] ツリー走査インタプリタの Result 化
- [ ] `__alloc` サイズ検証
- [ ] テストを catch_unwind から戻り値判定へ移行

## 問題点

以下はいずれも「想定されるドメイン上の失敗」だが `panic!` / `unwrap` になっている。既存記録（code-review-2026-04）は `interpret_global` の 1 箇所のみで、本件は未記録。

| 事象 | 箇所 |
|---|---|
| 0 除算 | `src/interpreter/vm/eval.rs:98`、`src/interpreter/exec.rs:596-599`（グローバル初期化 `let: x(1 / 0);` も含む） |
| assert 失敗 | `src/interpreter/vm/mod.rs:724,730`、`src/interpreter/exec.rs:162,169` |
| 不正アドレス・解放済みアクセス・二重 free | `src/interpreter/allocator.rs:114-125,144-161` |
| 評価回数上限超過 | `src/interpreter/environment.rs:126` |
| 出力エラー | `src/interpreter/environment.rs:139,144,148`（`write!(..).unwrap()`） |
| グローバル初期化失敗 | `src/lib.rs:97,113,141`（`expect("global initialization failed")`） |

release プロファイルは `panic = "abort"` のため、CLI は abort、WASM（NospaceVM）は trap して JS 側にエラー内容が届かない。テスト（`tests/code_test/error_base.rs:139`、`nospace_vm_base.rs:152`）は `catch_unwind` に依存しており、unwind が有効な test プロファイルでしか成立しない。

### 再現

- `nospace20 many.ns | head -c 10` → `environment.rs:144:41: ... BrokenPipe` で panic
- `__alloc(4611686018427387904)` → `capacity overflow` で panic
- `__alloc(9223372036854775806)` の後 `__alloc(4)` → `allocator.rs:301: attempt to add with overflow`
- さらに巨大なサイズでは OOM abort（catch 不能）

関連: 実行時エラー系 large テストはインタプリタと NospaceVM にしか生成されない（`src_build/nospace_tests.rs:352-380`）。WS ターゲットでの DivisionByZero / AssertionFailed は検証されていない。

## 違反している原則

- AGENTS.md「ドメイン上の想定される失敗は戻り値で返し、Panic は回復不能な異常のみ」

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `InterpreterAllocator::{get,set,free,alloc}` と `Environment::{write_*,flush}` を `Result<_, InterpretError>` 化し、`ExpressionFlow` / `Flow` に `Error` バリアントを追加して伝播。NospaceVM は `StepResult::Error` へ変換 | `interpreter/` 全体、`lib.rs`、`wasm_api/nospace_vm.rs`、テスト。大規模 |
| B | 段階的に、まず NospaceVM（WASM 経路）のみ Result 化 | `vm/eval.rs`、`vm/exec.rs`、`vm/mod.rs`、`allocator.rs` |
| C | `__alloc` のサイズ上限（負値・巨大値はエラー）と `next_addr` の `checked_add` | `allocator.rs`、`alloc_spec.rs`。WS 側 `alloc_runtime` と仕様を揃える |

推奨: C → B → A の順。[07-recursion-depth-limit.md](07-recursion-depth-limit.md) の案 A（CLI を NospaceVM に統一）を採るなら A の対象はほぼ NospaceVM のみになる。
