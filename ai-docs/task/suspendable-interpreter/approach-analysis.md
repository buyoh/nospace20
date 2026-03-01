# アプローチ比較

## 現状のインタプリタ構造

現在のインタプリタは**再帰呼び出し**で実行状態を管理している:

```
interpret_all()
  ├→ interpret_global()
  └→ interpret_func("__main")
       └→ LocalEnvironment::new_func()
       └→ interpret_statements([s1, s2, ...])
            └→ interpret_statement(s1)
                 └→ interpret_expression(expr)
                      └→ interpret_call_user_function_by_ref(func_ref, args)
                           └→ interpret_statements(...)    ← ネスト
                                └→ interpret_while_statement(...)
                                     └→ interpret_expression(...)
```

**実行状態** = Rust のコールスタック + `scope_stack`（スコープアドレス列）+ `Environment`（グローバル状態・アロケータ）

中断・再開するためには、この再帰的に積み上がった実行位置を保存・復元する必要がある。

## 参考: 既存の明示的スタックマシン (`WhitespaceVM`)

`src/whitespace/interpreter.rs` では Whitespace インタプリタが明示的スタックマシンとして実装済み:

```rust
pub struct WhitespaceVM {
    instructions: Vec<Instruction>,
    pc: usize,
    data_stack: Vec<i64>,
    call_stack: Vec<usize>,
    heap: HashMap<i64, i64>,
    stdin: StdinSource,
    stdout: Box<dyn Write>,
    completed: bool,
    total_steps: usize,
    // ...
}

impl WhitespaceVM {
    pub fn step(&mut self, budget: usize) -> StepResult { ... }
    pub fn run(&mut self, max_steps: usize) -> StepResult { ... }
    pub fn is_complete(&self) -> bool { ... }
    pub fn total_steps(&self) -> usize { ... }
    pub fn get_stdout_string(&self) -> String { ... }
}
```

Whitespace は命令列がフラットなため、プログラムカウンタ (`pc`) だけで状態を表現できる。
nospace は AST ツリーを解釈するため、フレームスタックで実行位置を管理する必要がある。

## アプローチ一覧

### A: 明示的スタックマシン化（新規モジュール）

再帰インタプリタを**変更せず残し**、新規モジュールとしてループ + 明示的スタックのインタプリタを追加する。

```rust
pub struct NospaceVM {
    scope: Scope,              // AST を所有
    frames: Vec<Frame>,        // 実行フレームスタック
    env: Environment,          // I/O・メモリ・メトリクス
    value_stack: Vec<i64>,     // 式評価のデータスタック
    completed: bool,
    total_steps: usize,
}

impl NospaceVM {
    pub fn step(&mut self, budget: usize) -> StepResult { ... }
    pub fn run(&mut self, max_steps: usize) -> StepResult { ... }
    pub fn is_complete(&self) -> bool { ... }
    pub fn total_steps(&self) -> usize { ... }
    pub fn get_stdout_string(&self) -> String { ... }
}
```

| 項目 | 評価 |
|------|------|
| 中断・再開 | ◎ 任意のタイミングで中断可能 |
| 実装コスト | △ 新規モジュールとして実装（既存コード変更なし） |
| 既存テスト | ◎ 既存コード無変更のため影響ゼロ |
| 保守性 | △ フレーム定義が複雑になりがち |
| WhitespaceVM との一貫性 | ◎ 同じ `step(budget) -> StepResult` パターン |
| WASM 統合 | ◎ Scope を所有するためライフタイム問題なし |

### B: Yield 伝播 + 継続保存 (ハイブリッド)

再帰構造を維持しつつ、`Flow::Yield` を追加して呼び出し元まで巻き戻す。

| 項目 | 評価 |
|------|------|
| 中断・再開 | ○ Yield 到達点で中断可能 |
| 実装コスト | △ 継続の保存が各メソッドに必要、既存コード全体に変更が波及 |
| 既存テスト | △ 再帰構造は維持するが、全メソッドに Yield 伝播が入り影響あり |
| 保守性 | △ 継続型が式・文の構造と密結合 |
| WhitespaceVM との一貫性 | ✕ インターフェースが異なる |
| WASM 統合 | △ ライフタイム問題（Scope 参照）を別途解決する必要がある |

### C: スレッド / Web Worker 委譲

インタプリタは変更せず、実行スレッドを分離する。

| 項目 | 評価 |
|------|------|
| 中断・再開 | △ Worker 終了 = 中断（途中再開は不可） |
| 実装コスト | ○ インタプリタ変更なし |
| 既存テスト | ◎ 完全に無影響 |
| 制約 | WASM 環境で SharedArrayBuffer + Atomics が必要（Cross-Origin Isolation 要求） |

### D: Async / Generator

Rust の async 関数として実行し、カスタム executor で N ステップごとに yield する。

| 項目 | 評価 |
|------|------|
| 中断・再開 | ◎ `Future::poll` で自然に中断・再開 |
| 実装コスト | △ 全関数を async 化する必要がある |
| 既存テスト | △ async テストランナー必要 |
| 制約 | `&mut Environment` を跨ぐ借用が困難（Pin + 自己参照問題） |

## 選定: アプローチ A (明示的スタックマシン化)

### 選定理由

1. **WhitespaceVM との一貫性** — `step(budget) -> StepResult` の同一インターフェースを提供でき、WASM ラッパー (`WasmNospaceVM`) も `WasmWhitespaceVM` と同パターンで実装可能
2. **既存コードへの影響ゼロ** — 再帰インタプリタを変更しないため、既存テストに一切影響しない。新規モジュールとして追加するだけ
3. **選択可能性** — 再帰版（高速・シンプル）とスタックマシン版（中断・再開可能）を用途に応じて使い分けられる
4. **WASM 統合の容易さ** — `Scope` を所有するためライフタイム問題がなく、WASM 境界をまたぐのが自然
5. **実績のあるパターン** — `WhitespaceVM` で同方式が実装済みであり、設計パターンが検証されている

### アプローチ B を不採用とした理由

- 既存の再帰インタプリタ全体に `Yield` 伝播を追加する必要があり、変更範囲が広い
- 継続情報の保存・復元が複雑で、各メソッドに密結合する
- `WhitespaceVM` と異なるインターフェースになり、WASM API の統一が困難
- 既存テストへの影響を完全に排除できない

### リスクと対策

| リスク | 対策 |
|--------|------|
| フレーム定義が複雑化 | AST ノード種類ごとに対応するフレーム型を定義。`ExecStatement` / `ExecExpression` のバリアントと1:1に近い対応を取る |
| 再帰版との動作差異 | 全既存テストを `NospaceVM` でも実行し、結果一致を検証する |
| パフォーマンス低下 | CLI ではデフォルトで再帰版を使用。スタックマシン版は WASM / ステップ実行が必要な場合のみ |
| `Scope` の所有コスト | `Scope` を move で渡す。共有が必要な場合は `Arc<Scope>` を検討 |
| AST への参照管理 | `Scope` を所有しているため、内部の AST ノードへの参照はインデックスベースで管理可能 |
