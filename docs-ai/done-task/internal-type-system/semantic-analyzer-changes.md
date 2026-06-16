# semantic_analyzer の変更設計

## 変更対象ファイル

- `src/semantic_analyzer/types.rs` — 型定義の追加
- `src/semantic_analyzer/scope.rs` — Function, FunctionIndex の拡張
- `src/semantic_analyzer/mod.rs` — 型推論・型チェックロジック

## 1. types.rs の変更

### ValueType enum の追加

```rust
/// 内部型システムの型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// 整数型（i64）
    Int,
    /// 値なし型
    Void,
}
```

### infer_type 関数の追加

`ExecExpression` から型を推論する関数を追加する。この関数は semantic_analyzer の式変換時と compiler_ws のコード生成時の両方で使用される。

```rust
impl ExecExpression {
    /// 式の型を推論する
    pub(crate) fn infer_type(&self, functions: &[Function]) -> ValueType {
        match self {
            ExecExpression::Factor(_) => ValueType::Int,
            ExecExpression::Variable(_) => ValueType::Int,
            ExecExpression::ArrayAccess(_, _, _) => ValueType::Int,
            ExecExpression::Operation1(_, _) => ValueType::Int,
            ExecExpression::Operation2(Operator2::Assign, _, rhs) => {
                // 代入式は右辺の型を返す（が、型チェック済みなら常に int）
                rhs.infer_type(functions)
            }
            ExecExpression::Operation2(_, _, _) => ValueType::Int,
            ExecExpression::While(_, _) => ValueType::Void,
            ExecExpression::If(_, then_block, else_block) => {
                // else が空ブロック（パーサーが生成する else なし if のデフォルト）→ void
                // 両方が int → int
                // いずれかが void → void
                infer_block_type(then_block, functions)
                    .merge(infer_block_type(else_block, functions))
            }
            ExecExpression::Block(block) => infer_block_type(block, functions),
            ExecExpression::BuiltinFunction(kind, _) => {
                match kind {
                    BuiltinFunctionKind::Trace => ValueType::Void,
                    _ => ValueType::Int, // puti, putc, geti, getc, clog, assert, assert_not
                }
            }
            ExecExpression::UserFunction(id_ref, _) => {
                // id_ref からグローバル関数インデックスを取得し、return_type を参照
                // ここでは functions スライスから取得
                functions[id_ref.local_index].return_type
            }
        }
    }
}

/// ブロックの型を推論する（最後の式文の型）
fn infer_block_type(block: &Block, functions: &[Function]) -> ValueType {
    match block.statements.last() {
        Some(ExecStatement::Expression(expr)) => expr.infer_type(functions),
        _ => ValueType::Void, // 空ブロック、または最後が return/break/continue
    }
}

impl ValueType {
    /// 2つの型をマージする（if/else の分岐統合用）
    fn merge(self, other: ValueType) -> ValueType {
        match (self, other) {
            (ValueType::Int, ValueType::Int) => ValueType::Int,
            _ => ValueType::Void,
        }
    }
}
```

注: `UserFunction` の `id_ref.local_index` は実際にはグローバル関数テーブルのインデックスであるため、正確なフィールド名は実装時に確認する。`FunctionIndex.0` がグローバルインデックスなので、`IdentifierRef` 経由でのアクセス方法を整理する必要がある。

### 代替案: ExecExpression に型タグを埋め込む

式に型情報を直接埋め込む方式はメモリオーバーヘッドが小さい（Copy 型の1バイト）が、全バリアントに追加する必要がある。推論関数方式のほうが既存コードへの影響が少ないため、推論関数方式を採用する。

## 2. scope.rs の変更

### Function 構造体

```rust
pub struct Function {
    pub arg_indices: Vec<usize>,
    pub block: Block,
    pub return_type: ValueType,  // 追加
}
```

### FunctionIndex の拡張

```rust
#[derive(Clone, Debug)]
pub(super) struct FunctionIndex(pub usize, pub usize, pub ValueType);
//                               global_idx  arg_count  return_type
```

### ScopeResolver への型クエリ追加

```rust
impl ScopeResolver {
    /// 関数の戻り値型を取得
    pub fn get_function_return_type(&self, name: &str) -> Option<ValueType> {
        // identifier_map を走査して FunctionIndex.2 を返す
    }
}
```

## 3. mod.rs の変更

### 関数の戻り値型推論（パス1a 拡張）

関数宣言のステートメントリストを再帰的にスキャンして、`return:` 文の有無を判定する。

```rust
/// 関数本体から return 文の有無を判定する
fn has_return_statement(statements: &[LocatedStatement]) -> bool {
    for stat in statements {
        match &stat.statement {
            Statement::Return(_) => return true,
            Statement::Expression(expr) => {
                if expr_contains_return(expr) {
                    return true;
                }
            }
            // ネストした関数宣言は除外（別の関数の return なので）
            Statement::FunctionDeclaration(_, _, _) => {}
            _ => {}
        }
    }
    false
}

/// 式の中に return を含むか（if/while/block 内の return をチェック）
fn expr_contains_return(expr: &Expression) -> bool {
    match expr {
        Expression::If(_, then_stmts, else_stmts) => {
            has_return_statement(then_stmts) || has_return_statement(else_stmts)
        }
        Expression::While(_, stmts) => has_return_statement(stmts),
        Expression::Block(stmts) => has_return_statement(stmts),
        _ => false,
    }
}
```

パス1a で `FunctionIndex` を登録する際に戻り値型も含める:

```rust
let return_type = if has_return_statement(body) {
    ValueType::Int
} else {
    ValueType::Void
};
scope.add_identifier(
    name,
    Identifier::Function(FunctionIndex(global_idx, args.len(), return_type)),
)?;
```

### 型チェックの挿入

`convert_to_exec_expression_with_resolver` で式を変換する際に、void チェックを行う。

**要チェック箇所一覧**:

| 箇所 | チェック内容 |
|------|-------------|
| `Operation2(Assign, lhs, rhs)` | rhs が void → エラー |
| `Operation2(非Assign, l, r)` | l または r が void → エラー |
| `Operation1(op, inner)` | inner が void → エラー |
| `Function(args)` | 引数が void → エラー |
| `If(cond, ...)` | cond が void → エラー（※ cond は通常リテラルか演算なので起きにくいが、関数呼び出しの場合あり） |
| `While(cond, ...)` | cond が void → エラー |
| `Return(expr)` | 関数が int で expr が void → エラー。関数が void で expr が指定 → エラー |
| `ArrayAccess(_, index)` | index が void → エラー |

**実装方針**: 各所で `require_int(expr, functions)` ヘルパーを呼び出す。

```rust
fn require_int(
    expr: &ExecExpression,
    functions: &[Function],
) -> Result<(), Vec<CodeParseError>> {
    if expr.infer_type(functions) == ValueType::Void {
        Err(vec![code_parse_error!(
            "semantic error: cannot use void expression as a value"
        )])
    } else {
        Ok(())
    }
}
```

### 関数宣言の本体解析後の整合性チェック

パス2（関数本体変換後）で、関数内の `return:` 文の式型と宣言された戻り値型の整合性を確認:

- int 関数 → return 文の式は int であること
- void 関数 → return 文が存在してはならない（存在すればパス1a で int と判定されるはずなので、ここでキャッチされるのは `return: <void式>;` のケースのみ）

### if/else チェインの else なし判定

パーサー（tree_parser）は else なしの `if` を空の else ブロック（`Vec::new()`）として表現する。semantic_analyzer では、else ブロックが空かどうかを確認して型を決定する:

```rust
Expression::If(cond, then_stmts, else_stmts) => {
    // else なし = else_stmts が空 → 全体は void
    if else_stmts.is_empty() {
        // 全体の型は void
    } else {
        // then と else の型をマージ
    }
}
```

### 制約: 型チェックのタイミング

型チェックは `ExecExpression` の構築後に行う。つまり:
1. 式を `ExecExpression` に変換（既存の処理）
2. 変換結果に対して型推論を実行
3. void 非許可の文脈であれば エラーを返す

これにより、既存の変換ロジックへの変更を最小限に抑える。

## 4. 検討事項

### `__trace` の void 化の影響

`__trace` を void にすると、`__trace(x);` の返り値を使用しているコードがエラーになる。実際にそのような使用パターンはないことを確認済み。

### `if: cond { expr; }` （else なし）の void 化

else なし if は全体が void になる。条件が真でもブロックの値は取得できない。これは意図的な設計で、else なし if は**副作用のためだけに使用される**式とする。

### 空ブロック `{}` の void 化

`{}` は最後の式文がないため void となる。現在 `y = {};` で `y == 0` をテストしている箇所があるが、void 代入としてエラーになる。テストの修正が必要。
