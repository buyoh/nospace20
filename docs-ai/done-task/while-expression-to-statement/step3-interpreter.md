# Step 3: interpreter の変更

## 概要

インタプリタで `while` の解釈を式レベルから文レベルに移動する。

## 変更内容

### 3-1. `interpret_expression` から While を削除

**ファイル**: `src/interpreter/exec.rs`

```rust
// 削除
ExecExpression::While(mode, cond, block) => self.interpret_while(mode, cond, block),
```

### 3-2. 文レベルの while 解釈を追加

**ファイル**: `src/interpreter/exec.rs`

`interpret_statement` または同等の文実行関数に `ExecStatement::While` の処理を追加。

現在の `interpret_while` メソッドは ExpressionFlow を返すが、文レベルでは Flow を返すように変更:

```rust
fn interpret_while_statement(
    &mut self,
    mode: &ConditionMode,
    cond: &Box<LocatedExecExpression>,
    block: &Block,
) -> Flow {
    loop {
        let cond_val = match self.interpret_expression(cond) {
            ExpressionFlow::Value(e) => e,
            ExpressionFlow::Jump(Flow::Return(x)) => return Flow::Return(x),
            ExpressionFlow::Jump(Flow::Continue) => panic!("..."),
            ExpressionFlow::Jump(Flow::Break) => panic!("..."),
            ExpressionFlow::Jump(Flow::Proceed) => panic!("..."),
        };
        let condition = match mode {
            ConditionMode::NonZero => cond_val != 0,
            ConditionMode::Zero => cond_val == 0,
            ConditionMode::Negative => cond_val < 0,
        };
        if !condition {
            break;
        }
        self.enter_block(&block.scope);
        let (flow, _value) = self.interpret_statements_with_value(&block.statements);
        match flow {
            Flow::Proceed | Flow::Continue => {
                self.leave_block();
            }
            Flow::Return(v) => {
                self.leave_block();
                return Flow::Return(v);
            }
            Flow::Break => {
                self.leave_block();
                break;
            }
        }
    }
    Flow::Proceed
}
```

### 3-3. 式レベルの `interpret_while` メソッドを削除

旧 `interpret_while` メソッドを削除。while は式ではなくなるため、`ExpressionFlow::Value(0)` を返す必要がなくなる。

## 確認ポイント

- while ループの条件評価が正しく動作すること
- break / continue が正しく動作すること
- return が while 内から正しく伝播すること
- while が値を返さないこと（文であるため）
