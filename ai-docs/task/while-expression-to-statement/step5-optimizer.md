# Step 5: optimizer の変更

## 概要

最適化パスで `ExecExpression::While` を参照している箇所を `ExecStatement::While` に移動する。

## 影響するパス

1. **condition_opt** - 条件式最適化（While の ConditionMode を変換）
2. **dead_code** - 未到達関数削除（While 内の関数呼び出しを走査）
3. **geti_opt** - geti/getc 最適化（While 内を再帰走査）
4. **tests** - テスト用の ConditionMode 置換ヘルパー

## 変更内容

### 5-1. condition_opt.rs

#### `optimize_expression` から While 処理を削除

以下の2つの match アームを削除:

```rust
// 削除
ExecExpression::While(ConditionMode::NonZero, mut cond, mut body) => { ... }
ExecExpression::While(mode, mut cond, mut body) => { ... }
```

#### `optimize_statement` に While 処理を追加

```rust
fn optimize_statement(stmt: &mut LocatedExecStatement) {
    match &mut stmt.statement {
        ExecStatement::Expression(expr) => optimize_located_expr(expr),
        ExecStatement::Return(Some(expr)) => optimize_located_expr(expr),
        ExecStatement::While(ref mut mode, ref mut cond, ref mut body) => {
            optimize_located_expr(cond);
            optimize_block(body);

            // NonZero の場合のみパターン変換を試みる
            if *mode == ConditionMode::NonZero {
                let cond_loc = cond.location.clone();
                let cond_expr = std::mem::replace(&mut cond.expression, ExecExpression::Factor(0));
                // optimize_while_nonzero の結果を分解して
                // mode / cond / body に書き戻す
                // (既存の optimize_while_nonzero は ExecExpression を返すので、
                //  ExecStatement::While に適合するようリファクタが必要)
            }
        }
        _ => {}
    }
}
```

**注意**: `optimize_while_nonzero` は現在 `ExecExpression::While(...)` を返す。
While が ExecStatement に移動するため、この関数のシグネチャを変更する必要がある。

#### `optimize_while_nonzero` のリファクタ

現在のシグネチャ:
```rust
fn optimize_while_nonzero(
    cond_expr: ExecExpression,
    cond_loc: SourceLocation,
    body: Block,
    loc: SourceLocation,
) -> ExecExpression
```

変更後: 個々の要素を返すようにする:
```rust
fn optimize_while_nonzero(
    cond_expr: ExecExpression,
    cond_loc: SourceLocation,
) -> (ConditionMode, ExecExpression)
```

戻り値は `(最適化されたモード, 最適化された条件式)` とし、
呼び出し元で `ExecStatement::While` の各フィールドに書き戻す。

### 5-2. dead_code.rs

#### `collect_called_in_expr` から While を削除

```rust
// 削除
ExecExpression::While(_, cond, body) => {
    collect_called_in_expr(&cond.expression, reachable, worklist);
    collect_called_in_block(body, reachable, worklist);
}
```

#### `collect_called_in_statement` に While を追加

```rust
fn collect_called_in_statement(
    stmt: &LocatedExecStatement,
    reachable: &mut HashSet<usize>,
    worklist: &mut VecDeque<usize>,
) {
    match &stmt.statement {
        ExecStatement::Expression(expr) => collect_called_in_expr(&expr.expression, reachable, worklist),
        ExecStatement::Return(Some(expr)) => collect_called_in_expr(&expr.expression, reachable, worklist),
        ExecStatement::While(_, cond, body) => {
            collect_called_in_expr(&cond.expression, reachable, worklist);
            collect_called_in_block(body, reachable, worklist);
        }
        _ => {}
    }
}
```

### 5-3. geti_opt.rs

#### `recurse_into_expr` から While を削除

```rust
// 削除
ExecExpression::While(_, cond, body) => {
    recurse_into_expr(cond);
    optimize_block(body);
}
```

#### 文レベルの処理を追加

`optimize_statement` 等の文レベル走査に While の再帰処理を追加:

```rust
fn optimize_statement(stmt: &mut LocatedExecStatement) {
    match &mut stmt.statement {
        ExecStatement::Expression(expr) => { ... }
        ExecStatement::Return(Some(expr)) => { ... }
        ExecStatement::While(_, cond, body) => {
            recurse_into_expr(cond);
            optimize_block(body);
        }
        _ => {}
    }
}
```

### 5-4. tests.rs

`replace_condition_modes` ヘルパーで `ExecExpression::While` を扱っている箇所を
`ExecStatement::While` に移動する。

```rust
// 削除
ExecExpression::While(ref mut m, cond, block) => { ... }

// 追加（文レベルのヘルパーに）
ExecStatement::While(ref mut m, cond, block) => { ... }
```

## 確認ポイント

- condition_opt: while の条件式最適化が正しく動作すること
- dead_code: while 内の関数呼び出しが到達可能としてマークされること
- geti_opt: while 内の `p = __geti()` パターンが最適化されること
- 全テストが通ること
