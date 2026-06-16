# NospaceVM コードレビュー指摘事項

コミット `1fbc3ce`, `64ce849`, `7ff32d4` で導入された NospaceVM 関連の変更に対するコードレビュー。
`.github/skills/update-code/SKILL.md` の注意事項に基づく。

## 対象ファイル

- `src/interpreter/vm.rs` (1854行, 新規)
- `src/wasm_api/nospace_vm.rs` (163行, 新規)
- `tests/code_test/nospace_vm_base.rs` (177行, 新規)
- `src/wasm_api/api.rs` (`run()` 関数削除)
- `src/wasm_api/types.rs` (`RunResult` 型削除)
- `src/wasm_api/whitespace_vm.rs` (`get_traced` 実装変更)
- `src_build/nospace_tests.rs`, `src_build/common.rs` (テスト生成追加)
- `tools/wasm-test/test.mjs` (WasmNospaceVM テスト追加)

## 指摘: 単一責任原則違反 (重大)

**SKILL.md:** 「単一責任原則に従い、モジュール・構造体を分割する」

`src/interpreter/vm.rs` が 1854 行で、`NospaceVM` 構造体の `impl` ブロックに以下の責務が混在している:

1. **構築・ビルダーパターン** (`from_source`, `from_scope`, `with_stdin`, `with_io`, `with_config`)
2. **実行制御** (`step`, `run`, `execute_one_step`)
3. **フロー制御伝播** (`propagate_flow`)
4. **グローバル初期化** (`step_global_init`, `set_global_phase`)
5. **関数フレーム管理** (`push_func_frame`)
6. **変数アクセス・スコープ管理** (`resolve_addr`, `get_variable`, `set_variable`, `enter_block`, `leave_scope`)
7. **static 変数保存・復元** (`save_static_vars`, `load_static_vars`)
8. **ブロック実行** (`step_exec_block`, `finish_exec_block`)
9. **式評価** (`step_eval_expr`, `eval_start`, `push_assign`, `finish_eval`, `set_eval_cont`) — 最大の責務
10. **組み込み関数実行** (`exec_builtin`, `exec_internal_builtin`)
11. **ループ実行** (`step_while`, `set_while_phase`, `step_for`, `set_for_phase`)

### 提案

少なくとも以下のように分離を検討:

- `vm.rs` → 構造体定義・構築・公開 API・実行制御
- `vm_eval.rs` → 式評価 (EvalCont, eval_start, step_eval_expr 等)
- `vm_exec.rs` → ステートメント実行 (ExecBlock, WhileLoop, ForLoop)
- `vm_scope.rs` → 変数アクセス・スコープ管理・static 変数

## 指摘: ドキュメントコメント不足 (中)

**SKILL.md:** 「構造体の概要は必ずドキュメントコメントとして追加する」

以下の enum にドキュメントコメント (`///`) が無い:

- `FlowControl` (L59)
- `GlobalInitPhase` (L66)
- `BlockCompletion` (L75)
- `ExecBlockWait` (L82)
- `WhilePhase` (L124)
- `ForPhase` (L134)
- `ExecuteResult` (L170)

`StepResult`, `EvalCont`, `Frame`, `NospaceVM` にはドキュメントコメントが付与されており一貫性が無い。

## 指摘: フィールドのカプセル化不足 (軽微)

`NospaceVM` の `traced` フィールドが `pub` だが、アクセサメソッド `traced()` が存在する。
テストからも `vm.traced()` メソッド経由でアクセスしており、`pub` は不要。

```rust
// 現状
pub traced: BTreeMap<i64, i64>,

// あるべき姿
traced: BTreeMap<i64, i64>,  // traced() メソッドでアクセス
```

## 指摘: 不要な `#[allow(dead_code)]` (軽微)

`ForPhase` enum に `#[allow(dead_code)]` が付与されている (L131) が、全バリアントが `step_for` で使用されている。不要な属性。

## 良い点

- Unit テストが `vm.rs` 内 `mod tests` に豊富に存在し（40+ テスト）、モジュールごとのテスト配置ルールに準拠
- `assert_vm_matches_interpreter` ヘルパーで再帰版インタプリタとの結果一致を検証しており品質が高い
- `nospace_vm_base.rs` の large テストで `test_ok_coding_base_vm`, `test_ok_coding_io_base_vm`, `test_runtime_error_base_vm` が実装されており、既存テストフレームワークと整合
- ドキュメントコメントが公開 API (`NospaceVM`, `StepResult`, `WasmNospaceVM`) には適切に付与
- 既存再帰インタプリタ (`exec.rs`) を変更せず維持しており、リスクが限定的

## ステータス

- [ ] 単一責任原則: モジュール分割
- [ ] ドキュメントコメント追加
- [ ] `pub traced` → `traced` + アクセサ
- [ ] `#[allow(dead_code)]` 削除
