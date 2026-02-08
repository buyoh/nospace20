# semantic_analyzer モジュール変更設計

## 対象ファイル

- `src/semantic_analyzer/mod.rs`

## 現状

### ExecExpression enum

```rust
pub enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    Function(String, Vec<Box<ExecExpression>>),
    Factor(i64),
    Variable(IdentifierRef),
}
```

### 式変換処理 (`convert_to_exec_expression_with_resolver`, L110-L165)

`Expression` → `ExecExpression` の変換。`Expression::Operation1` はそのまま `ExecExpression::Operation1` に変換される。`Operator1` は tree_parser と共有されているため、新しい `Operator1::Ref` / `Deref` バリアントは自動的に伝搬する。

### 変数解決 (`ScopeResolver`, L268-L295)

名前 → `IdentifierRef` 変換。`resolve_variable` でスコープスタックを逆順探索。

## 変更内容

### 1. `Operator1::Ref` の意味検証

`&` 演算子の対象が変数であることを検証する。tree_parser は `& expr` を一般的にパースするため、ここで制約を加える。

```rust
// convert_to_exec_expression_with_resolver 内
Expression::Operation1(Operator1::Ref, inner) => {
    match inner.as_ref() {
        Expression::Variable(name) => {
            let id_ref = resolver.resolve_variable(name)?;
            ExecExpression::Operation1(Operator1::Ref, Box::new(ExecExpression::Variable(id_ref)))
        }
        _ => {
            // エラー: & は変数に対してのみ使用可能
            return Err(/* semantic error */);
        }
    }
}
```

将来的に `&arr[i]` をサポートする場合、ここの検証を拡大する。

### 2. `Operator1::Deref` の変換

`*expr` は特に制約がない（任意の式の結果をアドレスとして解釈）。通常の `Operation1` 変換で対応可能。

```rust
Expression::Operation1(Operator1::Deref, inner) => {
    let exec_inner = convert_to_exec_expression_with_resolver(inner, resolver)?;
    ExecExpression::Operation1(Operator1::Deref, Box::new(exec_inner))
}
```

### 3. 代入の左辺検証

現在、意味解析段階で代入の左辺検証は行っていない（インタプリタの実行時に `ExecExpression::Variable` かチェック）。
`*ptr = value;` をサポートするため、この検証は既存のまま runtime で行う方針とする。

ただし将来的には意味解析で左辺値（lvalue）検証を行うべき。候補:

- `ExecExpression::Variable(...)` → OK
- `ExecExpression::Operation1(Operator1::Deref, ...)` → OK
- その他 → エラー

### 4. ExecExpression の変更は不要

`Operator1::Ref` / `Deref` は `ExecExpression::Operation1` で表現されるため、`ExecExpression` enum 自体への変更は不要。

## テスト

### ユニットテスト

```
let: x; &x;     → Operation1(Ref, Variable(IdentifierRef{...}))
let: p; *p;     → Operation1(Deref, Variable(IdentifierRef{...}))
&5;              → エラー（& の対象がリテラル）
&(x + 1);       → エラー（& の対象が式）
```
