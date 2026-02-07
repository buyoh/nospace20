# アプローチ比較

## 現状のインタプリタ構造

現在のインタプリタは**再帰呼び出し**で実行状態を管理している:

```
interpret()
  └→ interpret_func("main")
       └→ interpret_statements([s1, s2, ...])
            └→ interpret_statement(s1)
                 └→ interpret_expression(expr)
                      └→ interpret_call_user_function("foo")
                           └→ interpret_statements(...)    ← ネスト
                                └→ interpret_while(...)
                                     └→ interpret_expression(...)
```

**実行状態** = Rust のコールスタック + `LocalEnvironment`（変数値）+ `Environment`（グローバル状態）

中断・再開するためには、この再帰的に積み上がった実行位置を保存・復元する必要がある。

## アプローチ一覧

### A: 明示的スタックマシン化

再帰インタプリタをループ + 明示的スタックに書き換える。

```rust
enum Frame {
    EvalStatements { stmts: &[ExecStatement], index: usize, block_entered: bool },
    EvalExpression { expr: &ExecExpression, continuation: Continuation },
    EvalWhile { cond: &ExecExpression, block: &Block, phase: WhilePhase },
    EvalCall { func: &Function, args_evaluated: Vec<i64>, remaining: usize },
    // ...
}

struct InterpreterState {
    stack: Vec<Frame>,
    env: Environment,
    local_envs: Vec<LocalEnvironment>, // 関数呼び出しごと
}
```

| 項目 | 評価 |
|------|------|
| 中断・再開 | ◎ 任意のタイミングで中断可能 |
| 実装コスト | ✕ インタプリタ全体の書き直し |
| 既存テスト | △ 同じ結果だが内部構造が全く異なる |
| 保守性 | △ フレーム定義が複雑になりがち |

### B: Yield 伝播 + 継続保存 (ハイブリッド)

再帰構造を維持しつつ、`Flow::Yield` を追加して呼び出し元まで巻き戻す。
再開時は**継続情報**を使って中断地点まで早送りする。

```rust
enum Flow {
    Proceed,
    Return(i64),
    Continue,
    Break,
    Yield(Continuation), // ★追加
}
```

中断時:
1. `check_step_budget()` が `Yield` を返す
2. `Yield` が `try_expr!` 経由で全ての呼び出し元に伝播
3. 各レイヤーが自分の継続情報を `Continuation` に積む
4. 最上位に到達し、`Suspended(continuation)` を返す

再開時:
1. `Continuation` から実行位置を復元
2. 中断地点まで早送り（条件の再評価なし）
3. 通常の実行を再開

| 項目 | 評価 |
|------|------|
| 中断・再開 | ○ Yield 到達点で中断可能 |
| 実装コスト | △ 継続の保存が各メソッドに必要 |
| 既存テスト | ○ 再帰構造を維持するため既存ロジックへの影響が小さい |
| 保守性 | △ 継続型が式・文の構造と密結合 |

### C: スレッド / Web Worker 委譲

インタプリタは変更せず、実行スレッドを分離する。

```
[メインスレッド] ←メッセージ→ [Worker スレッド: interpret() 実行]
```

| 項目 | 評価 |
|------|------|
| 中断・再開 | △ Worker 終了 = 中断（途中再開は不可） |
| 実装コスト | ○ インタプリタ変更なし |
| 既存テスト | ◎ 完全に無影響 |
| 保守性 | ○ 関心の分離が明確 |
| 制約 | WASM 環境で SharedArrayBuffer + Atomics が必要（Cross-Origin Isolation 要求） |

### D: Async / Generator

Rust の async 関数として実行し、カスタム executor で N ステップごとに yield する。

```rust
async fn interpret_expression_async(...) -> ExpressionFlow {
    budget.check().await; // yield point
    // ...
}
```

| 項目 | 評価 |
|------|------|
| 中断・再開 | ◎ `Future::poll` で自然に中断・再開 |
| 実装コスト | △ 全関数を async 化する必要がある |
| 既存テスト | △ async テストランナー必要 |
| 保守性 | ○ async/await は Rust の標準パターン |
| 制約 | `&mut Environment` を跨ぐ借用が困難（Pin + 自己参照問題） |

## 選定: アプローチ B (Yield 伝播 + 継続保存)

### 選定理由

1. **再帰構造を維持** — 現在のインタプリタロジックをほぼそのまま残せる
2. **段階的導入** — Phase 2 で Yield 伝播のみ（panic 代替）、Phase 3 で継続保存と段階的に実装可能
3. **WASM 制約との相性** — SharedArrayBuffer 不要、async の自己参照問題なし
4. **テスト互換性** — 既存関数を内部で `budget=∞` で呼び出せば同じ動作

### リスクと対策

| リスク | 対策 |
|--------|------|
| 継続型が複雑化 | while/if/関数呼び出し の3パターンに限定。式の途中中断は行わない |
| try_expr! マクロの修正が広範囲 | Yield バリアントは既存の Jump と同じ経路で伝播するため、修正箇所は限定的 |
| 再開時の変数値の整合性 | LocalEnvironment の scope_stack を丸ごと保存（clone は重いが、ステップ頻度で制御） |
