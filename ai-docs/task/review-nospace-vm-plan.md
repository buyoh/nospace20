# NospaceVM コードレビュー改善 — 実装方針

## 概要

[review-nospace-vm.md](review-nospace-vm.md) のコードレビュー指摘事項に対する実装方針。
対象は `src/interpreter/vm.rs` (1854行) を中心とする NospaceVM 関連コード。

## 指摘事項と対応方針

### 1. 単一責任原則: モジュール分割 (重大)

#### 方針

`vm.rs` (単一ファイル) → `vm/` ディレクトリモジュールに変換し、責務ごとにファイルを分離する。
`semantic_analyzer/` の分割実績（[02-module-splitting.md](../done-task/code-design-review/02-module-splitting.md)）と同じパターンを適用。

#### ファイル構成

```
src/interpreter/vm/
├── mod.rs        # 型定義・NospaceVM 構造体・公開 API・実行制御
├── eval.rs       # 式評価 (EvalCont 関連メソッド)
├── exec.rs       # ステートメント実行・ループ実行
├── scope.rs      # 変数アクセス・スコープ管理・static 変数
└── tests.rs      # Unit テスト (#[cfg(test)] #[path = ...] パターン)
```

#### 各ファイルの責務

**`mod.rs`** (約 350 行):
- モジュール宣言 (`mod eval; mod exec; mod scope;`)
- raw pointer 型エイリアス (`StmtsPtr`, `ExprPtr`, `ArgsPtr`, `BlockPtr`)
- 全 enum / 型定義 (`FlowControl`, `GlobalInitPhase`, `BlockCompletion`, `ExecBlockWait`, `EvalCont`, `WhilePhase`, `ForPhase`, `Frame`, `ExecuteResult`, `StepResult`)
- `NospaceVM` 構造体定義
- 公開 API: コンストラクタ (`from_source`, `from_scope`)、ビルダー (`with_stdin`, `with_io`, `with_config`)、実行 (`step`, `run`)、状態参照 (`is_complete`, `total_steps`, `get_stdout_string`, `return_value`, `traced`, `flush`)
- 実行制御: `execute_one_step` (ディスパッチ)、`propagate_flow`
- グローバル初期化: `step_global_init`, `set_global_phase`, `push_func_frame`
- 組み込み関数: `exec_builtin`, `exec_internal_builtin`
- テストモジュール宣言 (`#[cfg(test)] #[path = "tests.rs"] mod tests;`)

**`eval.rs`** (約 250 行):
- `impl NospaceVM` ブロック:
  - `step_eval_expr` — 継続状態による式評価のディスパッチ
  - `eval_start` — 式の初期評価（ExecExpression のパターンマッチ）
  - `push_assign` — 代入式の処理
  - `finish_eval` — 式評価完了（フレーム pop + 値スタック push）
  - `set_eval_cont` — 継続状態の更新

**`exec.rs`** (約 280 行):
- `impl NospaceVM` ブロック:
  - `step_exec_block` — ExecBlock フレームのステップ実行
  - `finish_exec_block` — ExecBlock 完了時のスコープ・値処理
  - `step_while` — while ループのフェーズ駆動実行
  - `set_while_phase` — while フェーズ更新
  - `step_for` — for ループのフェーズ駆動実行
  - `set_for_phase` — for フェーズ更新

**`scope.rs`** (約 70 行):
- `impl NospaceVM` ブロック:
  - `resolve_addr` — IdentifierRef からメモリアドレスを解決
  - `get_variable` / `set_variable` — 変数の読み書き
  - `enter_block` / `leave_scope` — ブロックスコープの開始・終了
  - `save_static_vars` / `load_static_vars` — static 変数の保存・復元

**`tests.rs`** (約 450 行):
- 既存の `#[cfg(test)] mod tests` の内容をそのまま移動
- `exec.rs` / `exec_tests.rs` と同じ `#[path = ...]` パターンを使用

#### フィールド可視性の変更

`NospaceVM` の全プライベートフィールドを `pub(super)` に変更する。
子モジュール (`eval.rs`, `exec.rs`, `scope.rs`) が同一 `vm` モジュール内の兄弟として、フィールドにアクセス可能になる。

```rust
pub struct NospaceVM {
    pub(super) scope:          Scope,
    pub(super) frames:         Vec<Frame>,
    pub(super) value_stack:    Vec<i64>,
    pub(super) scope_stack:    Vec<i64>,
    pub(super) flow:           Option<FlowControl>,
    pub(super) env:            Environment,
    pub(super) stdout_capture: Option<Rc<RefCell<Vec<u8>>>>,
    pub(super) total_steps:    usize,
    traced:                    BTreeMap<i64, i64>,  // ← pub 削除、traced() メソッドでアクセス
    pub(super) completed:      bool,
    pub(super) return_value:   Option<i64>,
}
```

内部 enum 型も `pub(super)` に変更:

- `FlowControl`, `GlobalInitPhase`, `BlockCompletion`, `ExecBlockWait` → `pub(super)`
- `EvalCont`, `WhilePhase`, `ForPhase` → `pub(super)`
- `Frame`, `ExecuteResult` → `pub(super)`

#### mod.rs の use・再エクスポート

```rust
// mod.rs
mod eval;
mod exec;
mod scope;

// 外部に公開する型
pub use self::StepResult;
```

子モジュール側では:

```rust
// eval.rs, exec.rs, scope.rs
use super::*;  // mod.rs の型定義・use を全て引き継ぐ
```

### 2. ドキュメントコメント追加 (中)

以下の enum にドキュメントコメントを追加:

| enum | 追加するコメント |
|------|-----------------|
| `FlowControl` | `/// 制御フローの種別（return/break/continue）` |
| `GlobalInitPhase` | `/// グローバル初期化のフェーズ（static 変数初期化 → ルート文実行 → main 呼出し）` |
| `BlockCompletion` | `/// ExecBlock 完了時のアクション（スコープ解放・値 push の制御）` |
| `ExecBlockWait` | `/// ExecBlock のサブフレーム待機状態（式評価中・return 待ち等）` |
| `WhilePhase` | `/// while ループの実行フェーズ（条件評価 → チェック → ボディ実行）` |
| `ForPhase` | `/// for ループの実行フェーズ（init → cond → body → step の繰り返し）` |
| `ExecuteResult` | `/// execute_one_step の戻り値（VM 内部使用、Continue/Complete/Error）` |

### 3. `pub traced` → `traced` (軽微)

`NospaceVM` の `traced` フィールドから `pub` を削除。
既に `traced()` アクセサメソッドが存在し、全箇所で使用されている。
`wasm_api/nospace_vm.rs` の `get_traced` も `self.vm.traced()` メソッド経由。

注意: `traced` フィールドへの直接書き込みは `exec_builtin` 内の `self.traced.entry(a0).or_insert(0) += 1` のみ。
これは `NospaceVM` の `impl` ブロック内（mod.rs に残る）のため、`pub(super)` 不要で private のまま動作する。

ただし、`pub(super)` フィールドへの一括変更を行う場合、`traced` も `pub(super)` にしておき、
テストでの `vm.traced()` 使用は問題ない（メソッド経由）。

→ `traced` は private のままにする。ただし `exec_builtin` が mod.rs に存在するため問題なし。

### 4. `#[allow(dead_code)]` 削除 (軽微)

`ForPhase` enum の `#[allow(dead_code)]` (L131) を削除。
全バリアントが `step_for` で使用済み。

## 作業手順

1. **ドキュメントコメント追加** — `vm.rs` 内の 7 enum にコメント追加
2. **`#[allow(dead_code)]` 削除** — `ForPhase` の属性削除
3. **`pub traced` → private** — フィールドの `pub` 削除
4. **ファイル分割**:
   a. `vm.rs` → `vm/mod.rs` にリネーム（ディレクトリ `vm/` 作成）
   b. `vm/scope.rs` 作成 — スコープ関連メソッドを移動
   c. `vm/eval.rs` 作成 — 式評価メソッドを移動
   d. `vm/exec.rs` 作成 — ステートメント実行メソッドを移動
   e. `vm/tests.rs` 作成 — テストを移動
   f. `mod.rs` 内の NospaceVM フィールドを `pub(super)` に変更
   g. `mod.rs` 内に `mod` 宣言を追加
   h. 内部 enum を `pub(super)` に変更
5. **ビルド確認** — `cargo build` で全ファイルのコンパイルを確認
6. **テスト実行** — `cargo test` で既存テスト全パスを確認

## 備考

- `mod.rs` 内に `execute_one_step` (ディスパッチ) を残す理由: 各サブモジュールのステップ関数 (`step_eval_expr`, `step_exec_block`, etc.) を呼び出すハブであり、VM の実行制御の中核。`propagate_flow` も Frame 型全体にまたがるため mod.rs が適切。
- `push_func_frame` を mod.rs に残す理由: `eval.rs` (UserFuncArgs 完了時) と `step_global_init` (CallMain) の双方から呼ばれ、特定の責務に偏らない。
- テストファイルの `#[path]` パターン: `exec.rs` / `exec_tests.rs` の既存パターンに従い、vm ディレクトリモジュール内では `#[path = "tests.rs"]` を使用。
- `interpreter/mod.rs` の変更: `pub mod vm;` は既にファイルでもディレクトリでも動作するため変更不要。

## ステータス

- [x] ドキュメントコメント追加
- [x] `#[allow(dead_code)]` 削除
- [x] `pub traced` → private
- [x] ファイル分割 (vm.rs → vm/)
- [x] ビルド・テスト確認
