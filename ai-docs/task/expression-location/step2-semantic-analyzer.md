# Step 2: semantic_analyzer への LocatedExecExpression 導入

## 概要

`ExecExpression` をラップする `LocatedExecExpression` を導入し、
`LocatedExpression` の位置情報を意味解析後の中間表現まで引き継ぐ。

## 型定義の変更

### 新規型: `LocatedExecExpression`

```rust
// src/semantic_analyzer/types.rs

/// 位置情報付きの実行可能な式
pub(crate) struct LocatedExecExpression {
    pub expression: ExecExpression,
    pub location: SourceLocation,
}
```

### `ExecExpression` enum の変更

`Box<ExecExpression>` → `Box<LocatedExecExpression>` に変更:

```rust
// Before
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    Block(Block),
    BuiltinFunction(BuiltinFunctionKind, Vec<Box<ExecExpression>>),
    UserFunction(IdentifierRef, Vec<Box<ExecExpression>>),
    Factor(i64),
    Variable(IdentifierRef),
    ArrayAccess(IdentifierRef, Box<ExecExpression>, usize),
}

// After
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<LocatedExecExpression>),
    Operation2(Operator2, Box<LocatedExecExpression>, Box<LocatedExecExpression>),
    If(Box<LocatedExecExpression>, Block, Block),
    While(Box<LocatedExecExpression>, Block),
    Block(Block),
    BuiltinFunction(BuiltinFunctionKind, Vec<Box<LocatedExecExpression>>),
    UserFunction(IdentifierRef, Vec<Box<LocatedExecExpression>>),
    Factor(i64),
    Variable(IdentifierRef),
    ArrayAccess(IdentifierRef, Box<LocatedExecExpression>, usize),
}
```

変更なしのバリアント: `Block`, `Factor`, `Variable`

### `ExecStatement` enum の変更

```rust
// Before
pub(crate) enum ExecStatement {
    Return(Option<Box<ExecExpression>>),
    Break,
    Continue,
    Expression(Box<ExecExpression>),
}

// After
pub(crate) enum ExecStatement {
    Return(Option<Box<LocatedExecExpression>>),
    Break,
    Continue,
    Expression(Box<LocatedExecExpression>),
}
```

## convert_to_exec_expression_with_resolver の変更

### シグネチャ変更

```rust
// Before
fn convert_to_exec_expression_with_resolver(
    expr: &Box<Expression>,
    parent_resolver: &ScopeResolver,
    func_return_types: &[ValueType],
) -> Result<Box<ExecExpression>, Vec<CodeParseError>>

// After
fn convert_to_exec_expression_with_resolver(
    expr: &Box<LocatedExpression>,
    parent_resolver: &ScopeResolver,
    func_return_types: &[ValueType],
) -> Result<Box<LocatedExecExpression>, Vec<CodeParseError>>
```

### 変換パターン

入力の `LocatedExpression` から `location` を引き継ぎ、出力の `LocatedExecExpression` に設定する:

```rust
fn convert_to_exec_expression_with_resolver(
    located_expr: &Box<LocatedExpression>,
    parent_resolver: &ScopeResolver,
    func_return_types: &[ValueType],
) -> Result<Box<LocatedExecExpression>, Vec<CodeParseError>> {
    let loc = &located_expr.location;
    let expr = &located_expr.expression;

    let exec_expr = match expr {
        Expression::Factor(v) => ExecExpression::Factor(v.to_owned()),
        Expression::Variable(v) => {
            let var_ref = parent_resolver
                .resolve_variable(v)
                .ok_or_else(|| vec![code_parse_error!(loc.start, format!("undefined variable: {}", v))])?;
            ExecExpression::Variable(var_ref)
        }
        // ... 他のバリアントも同様
    };

    Ok(Box::new(LocatedExecExpression {
        expression: exec_expr,
        location: loc.clone(),
    }))
}
```

### エラー位置の改善

Phase 1 では意味解析エラーの `code_parse_error!` に位置情報なし（`None`）のものが多かった。
Phase 2 では `LocatedExpression.location.start` を利用して全てに位置を付与できる:

```rust
// Before (Phase 1)
Err(vec![code_parse_error!(format!("undefined variable: {}", v))])

// After (Phase 2)
Err(vec![code_parse_error!(loc.start, format!("undefined variable: {}", v))])
```

## require_int_type の変更

式の型チェック関数も `LocatedExecExpression` を受け取るよう変更:

```rust
// Before
fn require_int_type(
    expr: &ExecExpression,
    func_return_types: &[ValueType],
) -> Result<(), Vec<CodeParseError>>

// After
fn require_int_type(
    expr: &LocatedExecExpression,
    func_return_types: &[ValueType],
) -> Result<(), Vec<CodeParseError>>
```

`infer_type` は `ExecExpression` のメソッドのままで良い。`expr.expression.infer_type(...)` で呼び出す。

## infer_type / infer_block_type の変更

`ExecExpression::infer_type()` 内で再帰的に子式を参照する箇所は `LocatedExecExpression` 経由になる:

```rust
impl ExecExpression {
    pub(crate) fn infer_type(&self, func_return_types: &[ValueType]) -> ValueType {
        match self {
            ExecExpression::Operation2(Operator2::Assign, _, rhs) => {
                rhs.expression.infer_type(func_return_types)  // .expression を追加
            }
            // ...
        }
    }
}
```

## analyze_internal_with_parent の変更

パス2（文の変換）で `Statement` 内の式を `Box<LocatedExpression>` として受け取る:

```rust
Statement::VariableDeclaration(_, init, is_static_explicit, _) => {
    // init: &Box<LocatedExpression>  (変更後)
    let exec_expr = convert_to_exec_expression_with_resolver(
        init, &resolver, &effective_func_return_types,
    )?;
    let exec_stmt = ExecStatement::Expression(exec_expr);
    // ...
}
Statement::Return(Some(expr)) => {
    // expr: &Box<LocatedExpression>  (変更後)
    let exec_e = convert_to_exec_expression_with_resolver(
        expr, &resolver, &effective_func_return_types,
    )?;
    // ...
}
Statement::Expression(e) => {
    // e: &Box<LocatedExpression>  (変更後)
    exec_statements.push(LocatedExecStatement {
        statement: ExecStatement::Expression(
            convert_to_exec_expression_with_resolver(e, &resolver, &effective_func_return_types)?,
        ),
        location: loc.clone(),
    });
}
```

## has_return_statement / guarantees_return の変更

これらの関数は `Expression` を参照する。`LocatedExpression` 経由のアクセスに更新:

```rust
// Before
fn has_return_statement(statements: &Vec<LocatedStatement>) -> bool {
    for located in statements {
        match &located.statement {
            Statement::Expression(expr) => match expr.as_ref() {
                Expression::If(_, then_stmts, else_stmts) => { ... }

// After
fn has_return_statement(statements: &Vec<LocatedStatement>) -> bool {
    for located in statements {
        match &located.statement {
            Statement::Expression(located_expr) => match &located_expr.expression {
                Expression::If(_, then_stmts, else_stmts) => { ... }
```

## 変更対象ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `src/semantic_analyzer/types.rs` | `LocatedExecExpression` 定義、`ExecExpression` / `ExecStatement` 型変更、`infer_type` / `infer_block_type` 更新 |
| `src/semantic_analyzer/mod.rs` | `convert_to_exec_expression_with_resolver` 変更、`analyze_internal_with_parent` 変更、`has_return_statement` / `guarantees_return` 変更、`require_int_type` 変更 |
| `src/semantic_analyzer/scope.rs` | `Scope` の `static_init_statements` / `root_statements` は `LocatedExecStatement` のままなので変更なし |
| `src/semantic_analyzer/tests.rs` | テスト内の型利用を更新 |

## 留意事項

- `LocatedExecExpression` は `pub(crate)` とする（`ExecExpression` と同じ可視性）
- `LocatedExecExpression` にも `Clone` は不要（Phase 1 と同様の理由）
- 意味解析エラー (`code_parse_error!`) に位置をつける改善はこの Step のスコープ内で行う
  - `convert_to_exec_expression_with_resolver` 内で `loc.start` を使用
  - ただし、全ての `code_parse_error!` マクロに位置をつけることが目標ではなく、`LocatedExpression` から取得できる場合に限定
