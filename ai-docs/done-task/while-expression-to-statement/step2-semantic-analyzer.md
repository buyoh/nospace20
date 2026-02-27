# Step 2: semantic_analyzer の変更

## 概要

意味解析レベルで `while` を `ExecExpression` から `ExecStatement` に移動する。

## 変更内容

### 2-1. `ExecExpression` から `While` を削除

**ファイル**: `src/semantic_analyzer/types.rs`

```rust
// 削除
/// while 式: (条件モード, 条件式, ループ本体)
/// 意味解析では ConditionMode::NonZero で生成。最適化パスが Zero/Negative に変換可能。
While(ConditionMode, Box<LocatedExecExpression>, Block),
```

### 2-2. `ExecStatement` に `While` を追加

**ファイル**: `src/semantic_analyzer/types.rs`

```rust
pub(crate) enum ExecStatement {
    Return(Option<Box<LocatedExecExpression>>),
    Break,
    Continue,
    Expression(Box<LocatedExecExpression>),
    /// while 文: (条件モード, 条件式, ループ本体)
    /// 意味解析では ConditionMode::NonZero で生成。最適化パスが Zero/Negative に変換可能。
    While(ConditionMode, Box<LocatedExecExpression>, Block),
}
```

### 2-3. `infer_type` から While を削除

**ファイル**: `src/semantic_analyzer/types.rs`

`ExecExpression::infer_type` 内の以下を削除:
```rust
// 削除
ExecExpression::While(_, _, _) => ValueType::Void,
```

while は文であり、型推論の対象外となる。

### 2-4. 型システムドキュメント整合性

`docs/spec.md` の型テーブルから while を削除（Step 6 で実施）。

### 2-5. `convert_to_exec_expression_with_resolver` から While 処理を削除

**ファイル**: `src/semantic_analyzer/mod.rs`

`Expression::While` のマッチアームを削除（Expression に While がなくなるため自然に削除される）。

### 2-6. 文レベルの意味解析に While 処理を追加

**ファイル**: `src/semantic_analyzer/mod.rs`

`Statement::While` を文として処理する。文の変換処理は `analyze_internal_with_parent` 等の関数内で行う。

現在の `Expression::While` の処理ロジック:
```rust
Expression::While(expr, stat) => {
    let exec_cond = convert_to_exec_expression_with_resolver(expr, parent_resolver, func_return_types)?;
    require_int_type(&exec_cond, func_return_types)?;
    let (s, es) = analyze_internal_with_parent(
        stat, ScopeType::Block, Vec::new(), Some(parent_resolver),
        &mut Vec::new(), &mut Vec::new(), None,
        func_return_types.to_vec(),
    )?;
    Ok(make_located_exec(ExecExpression::While(
        ConditionMode::NonZero, exec_cond,
        Block { scope: s.build(Vec::new(), Vec::new(), Vec::new()), statements: es },
    ), loc))
}
```

これを文の処理として移動する。出力は `ExecStatement::While` となる。

### 2-7. `has_return_statement` の更新

**ファイル**: `src/semantic_analyzer/mod.rs`

`has_return_statement` 関数で `Expression::While` を走査していた箇所を更新。
while は Expression ではなくなるため、`Statement::While` レベルで走査する。

現在:
```rust
Expression::While(_, stmts) => has_return_statement(stmts),
```

変更後は Statement マッチで処理:
```rust
Statement::While(_, stmts) => has_return_statement(stmts),
```

## 確認ポイント

- `ExecExpression::While` を参照する全箇所がコンパイルエラーになること（Step 3-5 で対応）
- 条件式の void 型チェック（`require_int_type`）が引き続き動作すること
- `ConditionMode::NonZero` が初期値として正しく設定されること
