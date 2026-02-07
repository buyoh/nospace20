# 詳細設計

## 公開 API

### 型定義

```rust
// src/interpreter/session.rs (新規)

/// インタプリタの実行結果
pub enum StepResult {
    /// 実行完了
    Complete {
        return_value: Option<i64>,
    },
    /// 中断（ステップ数上限に到達）
    Suspended,
    /// エラー
    Error(String),
}

/// インタプリタの実行セッション
///
/// Scope への参照と実行状態を保持し、中断・再開を可能にする。
pub struct InterpreterSession<'a> {
    /// 解析済みスコープへの参照
    scope: &'a Scope,
    /// 実行環境（グローバル変数、I/O、メトリクス）
    env: Environment,
    /// 実行中の状態（None = 未開始 or 完了）
    continuation: Option<Continuation>,
    /// 1回の step() で実行する最大式評価回数
    step_budget: usize,
}
```

### メソッド

```rust
impl<'a> InterpreterSession<'a> {
    /// 新しいセッションを作成する
    pub fn new(scope: &'a Scope, env: Environment, step_budget: usize) -> Self;

    /// ステップを実行する
    ///
    /// step_budget 回の式評価を行い、以下のいずれかを返す:
    /// - Complete: 実行が完了した
    /// - Suspended: step_budget に到達し中断した
    /// - Error: 実行時エラーが発生した
    pub fn step(&mut self) -> StepResult;

    /// 実行が完了済みかどうか
    pub fn is_complete(&self) -> bool;

    /// 環境への参照を取得（stdout 等の読み取り用）
    pub fn env(&self) -> &Environment;

    /// 環境への可変参照を取得
    pub fn env_mut(&mut self) -> &mut Environment;
}
```

### lib.rs 公開 API

```rust
// src/lib.rs に追加

pub use interpreter::{InterpreterSession, StepResult};

/// セッションベースのインタプリタを開始する
pub fn interpret_session<'a>(
    scope: &'a Scope,
    env: Environment,
    step_budget: usize,
) -> InterpreterSession<'a> {
    InterpreterSession::new(scope, env, step_budget)
}
```

### 使用例 (native)

```rust
let scope = syntactic_analyze(&stmts)?;
let env = Environment::new();
let mut session = interpret_session(&scope, env, 10000);

loop {
    match session.step() {
        StepResult::Complete { return_value } => {
            println!("Done: {:?}", return_value);
            break;
        }
        StepResult::Suspended => {
            // 何かの処理（進捗表示等）
            continue;
        }
        StepResult::Error(msg) => {
            eprintln!("Error: {}", msg);
            break;
        }
    }
}
```

### 使用例 (WASM)

```javascript
const session = nospace.createSession(source, stdin, 10000);

function runChunk() {
  const result = session.step();
  if (result.status === 'suspended') {
    // UIフリーズ防止: 次のアニメーションフレームで続行
    requestAnimationFrame(runChunk);
  } else {
    // 完了 or エラー
    handleResult(result);
  }
}
runChunk();
```

## 内部設計

### Yield の伝播

`Flow` と `ExpressionFlow` に `Yield` バリアントを追加する:

```rust
#[derive(Debug)]
enum Flow {
    Proceed,
    Return(i64),
    Continue,
    Break,
    Yield,  // ★追加
}

enum ExpressionFlow {
    Value(i64),
    Jump(Flow),
    // Yield は Jump(Flow::Yield) として伝播
}
```

### ステップバジェットの管理

現在の `increment_expression_count` を改修する:

```rust
// 変更前
fn increment_expression_count(&mut self) {
    self.metrics.expression_count += 1;
    if let Some(max) = self.config.max_expression_count {
        if self.metrics.expression_count > max {
            panic!("Expression evaluation limit exceeded");
        }
    }
}

// 変更後
fn check_step_budget(&mut self) -> bool {
    self.metrics.expression_count += 1;
    self.remaining_budget = self.remaining_budget.saturating_sub(1);
    if self.remaining_budget == 0 {
        return false; // budget exhausted → should yield
    }
    // max_expression_count による絶対制限（安全弁）
    if let Some(max) = self.config.max_expression_count {
        if self.metrics.expression_count > max {
            return false;
        }
    }
    true // continue execution
}
```

### interpret_expression での Yield チェック

```rust
fn interpret_expression(&mut self, expr: &ExecExpression) -> ExpressionFlow {
    if !self.env.check_step_budget() {
        return ExpressionFlow::Jump(Flow::Yield);
    }
    // ... 既存のロジック
}
```

`try_expr!` マクロは `Jump` を伝播するため、`Yield` は自動的に呼び出し元まで伝播する:

```rust
macro_rules! try_expr {
    ($e: expr) => {
        match $e {
            ExpressionFlow::Value(x) => x,
            ExpressionFlow::Jump(f) => return ExpressionFlow::Jump(f),
            // ↑ Flow::Yield もここで伝播される
        }
    };
}
```

### Continuation (継続情報)

中断後の再開に必要な情報を保持する:

```rust
/// 実行の継続情報
///
/// 中断された地点を復元するために必要な全ての状態を保持する。
struct Continuation {
    /// コールスタック（再帰呼び出しの代わり）
    frames: Vec<ContinuationFrame>,
}

/// 1つの継続フレーム
enum ContinuationFrame {
    /// main 関数の実行 (or interpret() のルート)
    Root,

    /// 文リストの途中
    Statements {
        /// 実行中の文リストへの参照のための情報
        /// （再開時にどの文リストか特定するために使用）
        next_index: usize,
        /// ブロックスコープの変数値
        scope_values: Vec<i64>,
    },

    /// 関数呼び出しの途中
    FunctionCall {
        func_name: String,
        /// 評価済みの引数
        evaluated_args: Vec<i64>,
        /// 次に評価する引数のインデックス
        next_arg_index: usize,
    },

    /// while ループの途中
    WhileLoop {
        /// 現在の反復で条件は評価済みか
        condition_evaluated: bool,
        /// ブロック実行中の文インデックス
        body_next_index: usize,
        /// 最後のループ値
        last_value: i64,
    },

    /// if 式の途中
    IfBranch {
        /// 条件評価済みか
        condition_evaluated: bool,
        /// 条件の結果（true: then, false: else）
        condition_result: bool,
        /// ブロック実行中の文インデックス
        body_next_index: usize,
    },
}
```

### 中断時のデータフロー

```
interpret_expression が Yield を検知
  ↓
ExpressionFlow::Jump(Flow::Yield) を返す
  ↓
try_expr! が Jump を伝播
  ↓
interpret_statement が Flow::Yield を受け取る
  ↓ (各レイヤーが自分の状態を Continuation に push)
interpret_statements が Flow::Yield を受け取る
  ↓
interpret_func が Flow::Yield を受け取る
  ↓
InterpreterSession::step() が Suspended を返す
  ↓
呼び出し元 (JS / CLI) に制御が戻る
```

### 再開時のデータフロー

```
InterpreterSession::step() が呼ばれる
  ↓
Continuation から最外フレームを取り出す
  ↓
interpret_func を再呼び出し（復元情報付き）
  ↓
interpret_statements を途中のインデックスから再開
  ↓
式評価を通常通り実行
  ↓
budget 到達 or 完了
```

## 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/interpreter/mod.rs` | `Flow::Yield` 追加、`check_step_budget` 導入、各メソッドの Yield 伝播対応 |
| `src/interpreter/session.rs` | **新規**: `InterpreterSession`, `StepResult`, `Continuation` の定義と実装 |
| `src/lib.rs` | `interpret_session` 公開 API 追加、`InterpreterSession` / `StepResult` の re-export |

## 既存 API との互換性

既存の `interpret()` / `interpret_func()` はそのまま残す。内部的にセッションを使うがバジェット無制限で動作:

```rust
pub fn interpret(env: &mut Environment, scope: &Scope) -> Option<i64> {
    // 既存と同じ動作（バジェット無制限 = 中断なしで完了まで実行）
    // 内部実装は変更するが、外部動作は同一
}
```

**Result 型への移行**: 現在 `panic!` している `max_expression_count` 超過を `Flow::Yield` に変更することで、
パニックではなく正常な制御フローとして処理できるようになる。
ただし、`interpret()` / `interpret_func()` の戻り値型は互換性のため変えない。
超過時は `None` を返す（ステップ上限に達した場合は「main が値を返さなかった」扱い）。

## 段階的実装戦略

### Phase 1 でまず実現すること

- `InterpreterSession` の型定義と `step()` メソッドの骨格
- **中断はするが再開はしない** 状態（`step()` が `Suspended` を返したら、再度 `step()` すると最初から実行）
- これだけで「N ステップで止める」要件は満たせる

### Phase 2 で追加

- `Flow::Yield` の伝播
- panic の除去

### Phase 3 で追加

- `Continuation` による真の中断・再開
- これが最も難易度が高いが、Phase 1-2 が動いていれば段階的にテストしながら進められる

## 設計上のトレードオフ

### 式の途中での中断粒度

式 `a + foo(b * c)` の評価途中（`b * c` 評価後、`foo` 呼び出し前）で中断するかどうか。

**方針: 式の途中では中断しない**

- `interpret_expression` の**入口**でのみバジェットチェック
- 1つの式評価は原子的に完了する
- 中断粒度は「文の境界」「ループの反復境界」に限定

理由:
- 式の途中で中断すると、部分評価済みの中間値を全て保存する必要がある
- 実装の複雑さが大幅に増加
- 式1つの評価時間は十分短い（関数呼び出しを除く）

**ただし `interpret_call_user_function` は例外**: ユーザー定義関数呼び出しは内部で `interpret_statements` を実行するため、
関数本体の中での中断は自然に発生する（これは新しい `ContinuationFrame::FunctionCall` として保存される）。
