# Step 6: constexpr ブロック形式 詳細設計

## 概要

ブロック形式の constexpr 定義を実装する。
式形式の `constexpr: name(expr);` に加え、ブロック内の処理結果をコンパイル時定数として使用する。

**親ドキュメント**: [unimplemented-variable-features.md](../done-task/unimplemented-variable-features.md) §2.4, §4.7
**前提**: Step 2（constexpr 式形式）が実装済み

---

## 1. 構文

```nospace
constexpr: VALUE {
  let: tmp(3);
  tmp * tmp;   # = 9 #
};
```

- `constexpr: name { statements... };` でブロック形式の constexpr を定義
- ブロックの最後の式の値がコンパイル時定数 `name` の値になる
- ブロック内の全処理がコンパイル時に評価可能でなければコンパイルエラー
- 式形式 `constexpr: name(expr);` との混在・カンマ区切りは不可（ブロック形式では単一定義のみ）

### 1.1 ブロック内で許可される構文

| 構文 | 許可 | 備考 |
|------|------|------|
| `let: name(expr);` | ✅ | ローカル変数の宣言と初期化 |
| 代入 `name = expr;` | ✅ | ローカル変数への代入 |
| 複合代入 `+=`, `-=`, `*=`, `/=`, `%=` | ✅ | ローカル変数への複合代入 |
| 算術・比較・論理演算 | ✅ | `pure_eval` で既に対応済み |
| `if:` / `else:` | ✅ | コンパイル時の条件分岐 |
| ブロック式 `{ ... }` | ✅ | ネストしたブロックスコープ |
| 他の constexpr 参照 | ✅ | 既存のテーブルから解決 |
| リテラル | ✅ | 整数リテラル、16 進リテラル |

### 1.2 ブロック内で禁止される構文

| 構文 | 禁止理由 |
|------|----------|
| `while:`, `for:`, `repeat:` | 無限ループ検知の困難さ。将来拡張（反復上限付き）で対応可能 |
| 関数呼び出し `func()` | コンパイル時に関数本体を評価できない |
| 配列 `let: arr[N](0);` | コンパイル時配列管理の複雑さ |
| 参照 `&name`, `*name` | コンパイル時アドレスは無意味 |
| `static:`, `final:` | constexpr ブロック内に必要ない |
| `constexpr:`, `alias:` | ネスト禁止 |
| `func:` | constexpr ブロック内で関数定義不可 |
| `break:`, `continue:`, `return:` | ループ・関数がないため不要 |

---

## 2. アーキテクチャ設計

### 2.1 設計方針

コンパイル時ブロック評価器を `src/base/` に配置する。
これは `pure_eval.rs` と同様、**式の構造体に対する純粋な評価関数** を提供するモジュールとして設計する。

```
src/base/
├── mod.rs
├── location.rs
├── pure_eval.rs          # 既存: 二項/単項演算の純粋評価
└── constexpr_eval.rs     # 新規: constexpr ブロックの評価器
```

### 2.2 モジュール分割の理由

- `pure_eval.rs` が `base/` にある理由と同じ: コンパイル時評価は semantic_analyzer・optimizer・interpreter いずれにも属さない汎用機能
- `semantic_analyzer/mod.rs` の `evaluate_constexpr_expr()` は constexpr テーブル管理（ホイスティング・巡回検知）に特化しており、**式レベルの評価ロジック** と **テーブル管理ロジック** を分離できる
- 将来的に `evaluate_constexpr_expr()` 内のロジック（式評価部分）も `base/constexpr_eval.rs` に移行できる

### 2.3 依存関係

```
base/constexpr_eval.rs
    ├── depends on: base/pure_eval.rs (eval_binary_pure, eval_unary_pure, bool_to_int)
    ├── depends on: tree_parser (Expression, Statement, Operator1, Operator2, Located*)
    └── depends on: base/mod.rs (CodeParseError, SourceLocation)

semantic_analyzer/mod.rs
    ├── uses: base/constexpr_eval.rs (eval_constexpr_block, eval_constexpr_expr_with_env)
    └── manages: constexpr table, hoisting, cycle detection (既存責務)
```

---

## 3. `src/base/constexpr_eval.rs` 詳細設計

### 3.1 データ構造

```rust
use std::collections::BTreeMap;
use crate::base::{CodeParseError, SourceLocation};
use crate::tree_parser::expression::*;
use crate::tree_parser::statement::*;

/// constexpr ブロック評価用の環境
///
/// ブロック内ローカル変数と外部 constexpr テーブルの参照を保持する。
/// ブロック式のネストに対応するため、環境をスタック的に管理する。
pub struct ConstexprEnv<'a> {
    /// 外側の constexpr テーブル（読み取り専用）
    constexpr_table: &'a BTreeMap<String, i64>,
    /// ローカル変数スコープのスタック
    /// 最後の要素が現在のスコープ
    scopes: Vec<BTreeMap<String, i64>>,
}
```

### 3.2 環境操作

```rust
impl<'a> ConstexprEnv<'a> {
    /// 新しい環境を作成する
    pub fn new(constexpr_table: &'a BTreeMap<String, i64>) -> Self {
        Self {
            constexpr_table,
            scopes: vec![BTreeMap::new()],
        }
    }

    /// 新しいスコープを開く（ブロック式のネスト用）
    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    /// 現在のスコープを閉じる
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// 変数を検索する（内側のスコープから順に探索）
    fn get_variable(&self, name: &str) -> Option<i64> {
        // ローカル変数を内側から探索
        for scope in self.scopes.iter().rev() {
            if let Some(&v) = scope.get(name) {
                return Some(v);
            }
        }
        // constexpr テーブルから探索
        self.constexpr_table.get(name).copied()
    }

    /// 現在のスコープに変数を宣言する
    fn declare_variable(&mut self, name: String, value: i64) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    /// 既存の変数に代入する（最も内側のスコープで見つかったものを更新）
    fn assign_variable(&mut self, name: &str, value: i64) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        false // 変数が見つからない
    }
}
```

### 3.3 式評価関数

```rust
/// constexpr 環境内で式を評価する
///
/// 既存の `evaluate_constexpr_expr()` と異なり、ローカル変数を含む環境上で動作する。
/// `evaluate_constexpr_expr()` は raw/resolved/evaluating の 3 つのテーブルを使うが、
/// この関数は事前解決済みの ConstexprEnv 上で動作する。
pub fn eval_constexpr_expr(
    expr: &LocatedExpression,
    env: &ConstexprEnv,
) -> Result<i64, Vec<CodeParseError>> {
    let loc = expr.location.start;
    match &expr.expression {
        Expression::Factor(n) => Ok(*n),

        Expression::Variable(name) => {
            env.get_variable(name).ok_or_else(|| {
                vec![code_parse_error!(loc, format!("'{}' is not defined in constexpr block", name))]
            })
        }

        Expression::Operation1(op, inner) => {
            let v = eval_constexpr_expr(inner, env)?;
            match op {
                Operator1::Negative => Ok(v.wrapping_neg()),
                Operator1::LogicalNot => Ok(pure_eval::bool_to_int(v == 0)),
                _ => Err(vec![code_parse_error!(loc, "Ref/Deref is not allowed in constexpr block")]),
            }
        }

        Expression::Operation2(op, l, r) => {
            match op {
                // 代入演算は式としてではなく Statement 経由で処理する
                Operator2::Assign | Operator2::PlusAssign | Operator2::MinusAssign
                | Operator2::MultiplyAssign | Operator2::DivideAssign
                | Operator2::ModuloAssign => {
                    Err(vec![code_parse_error!(loc, "assignment expression is not supported in constexpr block")])
                }
                // 短絡評価
                Operator2::LogicalAnd => {
                    let lv = eval_constexpr_expr(l, env)?;
                    if lv == 0 { return Ok(0); }
                    let rv = eval_constexpr_expr(r, env)?;
                    Ok(pure_eval::bool_to_int(rv != 0))
                }
                Operator2::LogicalOr => {
                    let lv = eval_constexpr_expr(l, env)?;
                    if lv != 0 { return Ok(1); }
                    let rv = eval_constexpr_expr(r, env)?;
                    Ok(pure_eval::bool_to_int(rv != 0))
                }
                // その他の純粋演算
                _ => {
                    let lv = eval_constexpr_expr(l, env)?;
                    let rv = eval_constexpr_expr(r, env)?;
                    pure_eval::eval_binary_pure(op, lv, rv).ok_or_else(|| {
                        vec![code_parse_error!(loc, "division by zero in constexpr block")]
                    })
                }
            }
        }

        Expression::If(cond, then_body, else_body) => {
            eval_constexpr_if(cond, then_body, else_body, env)
        }

        Expression::Block(stmts) => {
            eval_constexpr_block(stmts, env)  // env 内でスコープを push/pop
        }

        _ => Err(vec![code_parse_error!(loc, "expression is not compile-time evaluable in constexpr block")]),
    }
}
```

### 3.4 ブロック評価関数

```rust
/// constexpr ブロックを評価する
///
/// ブロック内の文を順に実行し、最後の式の値を返す。
/// 新しいスコープを開き、ブロック終了時に閉じる。
pub fn eval_constexpr_block(
    statements: &[LocatedStatement],
    env: &mut ConstexprEnv,
) -> Result<i64, Vec<CodeParseError>> {
    env.push_scope();
    let result = eval_constexpr_block_inner(statements, env);
    env.pop_scope();
    result
}

fn eval_constexpr_block_inner(
    statements: &[LocatedStatement],
    env: &mut ConstexprEnv,
) -> Result<i64, Vec<CodeParseError>> {
    let mut last_value: Option<i64> = None;

    for stmt in statements {
        match &stmt.statement {
            Statement::VariableDeclaration(name, init, is_static, is_final, array_size) => {
                // static, final, array はコンパイル時ブロック内では禁止
                if *is_static {
                    return Err(vec![code_parse_error!(
                        stmt.location.start, "static variables are not allowed in constexpr block"
                    )]);
                }
                if *is_final {
                    return Err(vec![code_parse_error!(
                        stmt.location.start, "final variables are not allowed in constexpr block"
                    )]);
                }
                if array_size.is_some() {
                    return Err(vec![code_parse_error!(
                        stmt.location.start, "arrays are not allowed in constexpr block"
                    )]);
                }
                let value = eval_constexpr_expr(init, env)?;
                env.declare_variable(name.clone(), value);
                last_value = Some(value);
            }

            Statement::Expression(expr) => {
                // 代入文（式文として現れる a = b; の形）を先にチェック
                if let Expression::Operation2(op, lhs, rhs) = &expr.expression {
                    if matches!(op,
                        Operator2::Assign | Operator2::PlusAssign | Operator2::MinusAssign
                        | Operator2::MultiplyAssign | Operator2::DivideAssign | Operator2::ModuloAssign
                    ) {
                        let rhs_value = eval_constexpr_expr(rhs, env)?;
                        eval_constexpr_assign(op, lhs, rhs_value, env, stmt.location.start)?;
                        last_value = Some(rhs_value);
                        continue;
                    }
                }
                let value = eval_constexpr_expr(expr, env)?;
                last_value = Some(value);
            }

            _ => {
                return Err(vec![code_parse_error!(
                    stmt.location.start,
                    "unsupported statement in constexpr block"
                )]);
            }
        }
    }

    last_value.ok_or_else(|| vec![code_parse_error!("constexpr block has no value")])
}
```

### 3.5 代入処理

```rust
/// constexpr ブロック内での代入を処理する
///
/// NOTE: 代入式は tree_parser では Expression::Operation2(Assign, lhs, rhs) として解析される。
/// eval_constexpr_expr では代入を拒否するが、Statement::Expression として現れた場合には
/// この関数で処理する。
fn eval_constexpr_assign(
    op: &Operator2,
    target: &LocatedExpression,
    rhs_value: i64,
    env: &mut ConstexprEnv,
    loc: usize,
) -> Result<(), Vec<CodeParseError>> {
    let name = match &target.expression {
        Expression::Variable(name) => name,
        _ => return Err(vec![code_parse_error!(loc, "invalid assignment target in constexpr block")]),
    };

    // constexpr テーブルの値への代入は禁止
    if env.constexpr_table.contains_key(name) {
        return Err(vec![code_parse_error!(
            loc, format!("cannot assign to constexpr constant '{}'", name)
        )]);
    }

    let new_value = match op {
        Operator2::Assign => rhs_value,
        Operator2::PlusAssign | Operator2::MinusAssign
        | Operator2::MultiplyAssign | Operator2::DivideAssign
        | Operator2::ModuloAssign => {
            let old = env.get_variable(name).ok_or_else(|| {
                vec![code_parse_error!(loc, format!("'{}' is not defined", name))]
            })?;
            let base_op = match op {
                Operator2::PlusAssign => Operator2::Plus,
                Operator2::MinusAssign => Operator2::Minus,
                Operator2::MultiplyAssign => Operator2::Multiply,
                Operator2::DivideAssign => Operator2::Divide,
                Operator2::ModuloAssign => Operator2::Modulo,
                _ => unreachable!(),
            };
            pure_eval::eval_binary_pure(&base_op, old, rhs_value).ok_or_else(|| {
                vec![code_parse_error!(loc, "division by zero in constexpr block")]
            })?
        }
        _ => unreachable!(),
    };

    if !env.assign_variable(name, new_value) {
        return Err(vec![code_parse_error!(
            loc, format!("'{}' is not defined in constexpr block", name)
        )]);
    }
    Ok(())
}
```

### 3.6 if 式評価

```rust
/// constexpr ブロック内の if 式を評価する
fn eval_constexpr_if(
    cond: &LocatedExpression,
    then_body: &[LocatedStatement],
    else_body: &[LocatedStatement],
    env: &mut ConstexprEnv,
) -> Result<i64, Vec<CodeParseError>> {
    let cond_value = eval_constexpr_expr(cond, env)?;
    if cond_value != 0 {
        eval_constexpr_block(then_body, env)
    } else if !else_body.is_empty() {
        eval_constexpr_block(else_body, env)
    } else {
        Ok(0)  // else なしの if: 偽のとき 0
    }
}
```

### 3.7 公開 API まとめ

```rust
// src/base/constexpr_eval.rs の公開 API
pub struct ConstexprEnv<'a> { ... }
impl ConstexprEnv { pub fn new(...) -> Self; }
pub fn eval_constexpr_expr(expr, env) -> Result<i64, Vec<CodeParseError>>;
pub fn eval_constexpr_block(stmts, env) -> Result<i64, Vec<CodeParseError>>;
```

---

## 4. パーサー変更 (`src/tree_parser/statement/mod.rs`)

### 4.1 Statement enum の方針

**方法B を採用**: 既存の `ConstexprDeclaration(String, Box<LocatedExpression>)` をそのまま利用する。

ブロック形式の場合、式部分に `Expression::Block(Vec<LocatedStatement>)` を格納する。
tree_parser レベルでは式形式とブロック形式を同じ `ConstexprDeclaration` で表現する。

- **式形式**: `constexpr: V(1+2);` → `ConstexprDeclaration("V", Factor/Operation2/...)`
- **ブロック形式**: `constexpr: V { let: x(3); x*x; };` → `ConstexprDeclaration("V", Block(stmts))`

### 4.2 パーサー分岐

`parse_constexpr_declarations()` の修正:

```rust
// 識別子取得後、次トークンで分岐:
match self.iter.peek() {
    Some((Token::ParenthesisL, _)) => {
        // 既存の式形式パース（現状と同じ）
        self.iter.next(); // '(' を消費
        let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
        self.code_parse_error.append(&mut errs);
        match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);
        // ... ConstexprDeclaration を生成
    }
    Some((Token::BraceL, _)) => {
        // ブロック形式パース（AliasBlock と同様のパターン）
        let body = self.parse_to_statements_block();
        let end_pos = self.current_pos_or(start_pos);
        let loc = SourceLocation::new(start_pos, end_pos);
        results.push(LocatedStatement {
            statement: Statement::ConstexprDeclaration(
                id.to_string(),
                Box::new(LocatedExpression {
                    expression: Expression::Block(body),
                    location: loc,
                }),
            ),
            location: loc,
        });
        // ブロック形式は単一定義 → ループを抜けてセミコロンへ
        break;
    }
    _ => { /* エラー処理 */ }
}
```

### 4.3 カンマ区切りの扱い

- **式形式**: 従来通り `constexpr: A(1), B(2);` のカンマ区切り複数定義に対応
- **ブロック形式**: 単一定義のみ。カンマ区切りは不可（`alias:` ブロック形式と同じ制約）
- ブロック形式の後はカンマチェックをスキップして直接セミコロンを消費

---

## 5. 意味解析変更 (`src/semantic_analyzer/mod.rs`)

### 5.1 `collect_constexpr_table()` の変更

変更不要。`evaluate_constexpr_by_name()` 経由で呼ばれるため。

### 5.2 `evaluate_constexpr_by_name()` の変更

`Expression::Block` を検出した場合、`base::constexpr_eval::eval_constexpr_block` を呼び出す。

```rust
fn evaluate_constexpr_by_name(
    name: &str,
    raw: &BTreeMap<String, Box<LocatedExpression>>,
    resolved: &mut BTreeMap<String, i64>,
    evaluating: &mut BTreeSet<String>,
) -> Result<i64, Vec<CodeParseError>> {
    if let Some(&v) = resolved.get(name) {
        return Ok(v);
    }
    if evaluating.contains(name) {
        return Err(vec![code_parse_error!(...)]);
    }
    evaluating.insert(name.to_string());
    let expr = raw.get(name).unwrap();

    let value = match &expr.expression {
        Expression::Block(stmts) => {
            // ブロック形式: base/constexpr_eval を使用
            let mut env = constexpr_eval::ConstexprEnv::new(resolved);
            constexpr_eval::eval_constexpr_block(stmts, &mut env)?
        }
        _ => {
            // 式形式: 既存のロジック
            evaluate_constexpr_expr(expr, raw, resolved, evaluating)?
        }
    };

    evaluating.remove(name);
    resolved.insert(name.to_string(), value);
    Ok(value)
}
```

### 5.3 ブロック形式での前方参照

**課題**: ブロック形式 constexpr 内で他の constexpr を参照する場合、それが未解決の可能性がある。

**方針**: `ConstexprEnv` に渡す `constexpr_table` は `resolved`（解決済みテーブル）を使用する。
ブロック内から未解決の constexpr を参照した場合:

1. `env.get_variable(name)` が `resolved` から見つけられない
2. `eval_constexpr_expr` がエラーを返す

**採用方針（簡易方式）**: ブロック内から他の constexpr を参照する場合は、参照先が先に定義・解決済みであることを要求する。未解決ならエラー。

**理由**:
- 式形式 constexpr では lazy 解決がホイスティング対応のために必要だったが、ブロック形式ではブロック内からの外部参照は限定的
- ブロック評価前に `evaluate_constexpr_by_name` のループで他の constexpr は全て解決済み（or 評価中）
- `resolved` テーブルを `ConstexprEnv` に渡すだけで十分

ただし、ブロック形式 constexpr が別のブロック形式 constexpr を参照する場合、解決順序に注意が必要。
`evaluate_constexpr_by_name` の再帰呼び出しが先に行われるため、`resolved` には呼び出し時点で解決済みの値が入っている。

---

## 6. 代入式の処理設計

### 6.1 課題: 代入は式として解析される

nospace では `a = expr` は `Expression::Operation2(Assign, Variable("a"), expr)` として解析される。
つまり `a = 3;` は `Statement::Expression(Operation2(Assign, ...))` になる。

### 6.2 constexpr ブロック内での代入処理

`eval_constexpr_expr` は代入式を拒否する（値を返す式として不適切）。
代入は `eval_constexpr_block_inner` の `Statement::Expression` 分岐で特別に処理する:

1. `Statement::Expression` の式が `Operation2(Assign/xxxAssign, ...)` の場合
2. rhs を `eval_constexpr_expr` で評価
3. `eval_constexpr_assign` で lhs の変数に代入
4. `last_value` を更新（代入の結果値）

**注意**: この設計では代入式のネスト（`a = b = 3`）は簡易的にはサポートしない。

### 6.3 代入処理の実装詳細

代入文 `a = expr;` の AST:
```
Statement::Expression(
    Operation2(Assign,
        Variable("a"),
        <expr>
    )
)
```

`eval_constexpr_block_inner` での処理フロー:
1. 式が `Operation2(Assign, lhs, rhs)` かチェック
2. rhs を `eval_constexpr_expr(rhs, env)` で評価 → `rhs_value`
3. lhs が `Variable(name)` かチェック
4. `env.assign_variable(name, rhs_value)` で代入
5. 複合代入（`+=` 等）は old value を取得して `eval_binary_pure` で計算後に代入

---

## 7. テスト計画

### 7.1 Unit テスト (`src/base/constexpr_eval.rs` 内)

| テスト | 内容 |
|--------|------|
| `test_eval_expr_factor` | リテラル評価 |
| `test_eval_expr_arithmetic` | 四則演算 |
| `test_eval_expr_variable` | 環境内変数参照 |
| `test_eval_expr_constexpr_ref` | constexpr テーブルからの参照 |
| `test_eval_block_let` | `let:` 変数宣言 |
| `test_eval_block_assign` | 変数代入 |
| `test_eval_block_compound_assign` | 複合代入 `+=` 等 |
| `test_eval_block_if` | if 式 |
| `test_eval_block_nested_scope` | ネストしたブロックスコープ |
| `test_eval_block_no_value_error` | 空ブロックのエラー |
| `test_eval_block_static_error` | static 禁止エラー |
| `test_eval_block_array_error` | 配列禁止エラー |

### 7.2 Large テスト (`resources/tests/`)

| ファイル | 内容 |
|----------|------|
| `variables/constexpr_block_basic_001` | 基本ブロック形式 |
| `variables/constexpr_block_let_001` | let 変数を使用した計算 |
| `variables/constexpr_block_if_001` | if 分岐を含むブロック |
| `variables/constexpr_block_nested_001` | ネストしたブロック |
| `variables/constexpr_block_assign_001` | 変数代入を含むブロック |
| `variables/constexpr_block_mixed_001` | 式形式と混在 |
| `fails/compile/constexpr_block_loop_001` | while 禁止エラー |
| `fails/compile/constexpr_block_func_001` | 関数呼び出し禁止エラー |
| `fails/compile/constexpr_block_empty_001` | 空ブロックエラー |

---

## 8. 実装ステップ

### Step 6a: `src/base/constexpr_eval.rs` の実装

1. `ConstexprEnv` 構造体とスコープ管理メソッドを実装
2. `eval_constexpr_expr()` を実装（`pure_eval` を利用）
3. `eval_constexpr_block()` を実装
4. `eval_constexpr_assign()` を実装
5. `eval_constexpr_if()` を実装
6. Unit テストを追加

### Step 6b: パーサーのブロック形式対応

1. `parse_constexpr_declarations()` に `{` 分岐を追加
2. `Expression::Block` でラップして `ConstexprDeclaration` を生成
3. パーサーテストを追加

### Step 6c: 意味解析の統合

1. `evaluate_constexpr_by_name()` で `Expression::Block` を検出してブロック評価器を呼び出す
2. `collect_constexpr_table()` は変更不要（`evaluate_constexpr_by_name` 経由で呼ばれる）
3. 統合テストを追加

### Step 6d: Large テスト追加

1. 成功ケースのテスト追加
2. エラーケースのテスト追加
3. `docs/spec.md` へのブロック形式 constexpr の仕様追記

---

## 9. 既存の `evaluate_constexpr_expr()` との関係

### 9.1 移行計画

現在 `semantic_analyzer/mod.rs` にある `evaluate_constexpr_expr()` の **式評価ロジック** は、
`base/constexpr_eval.rs` の `eval_constexpr_expr()` と重複する。

**Phase 1（Step 6 での実装）**:
- `base/constexpr_eval.rs` にブロック評価器を新設
- 式形式 constexpr は既存の `evaluate_constexpr_expr()` をそのまま使用
- ブロック形式 constexpr のみ新しい `eval_constexpr_block()` を使用

**Phase 2（将来のリファクタリング）**:
- 式形式 constexpr も `eval_constexpr_expr()` with `ConstexprEnv` を使用するよう統合
- `semantic_analyzer/mod.rs` の `evaluate_constexpr_expr()` を廃止
- `evaluate_constexpr_by_name()` が `ConstexprEnv` ベースの評価に統一

### 9.2 `constant_folding` との関係

optimizer の `constant_folding` は **AST 変換**（式ノードをリテラルに置換）を行う。
constexpr ブロック評価は **値の計算**（ブロックの実行結果を i64 として返す）を行う。

両者は目的が異なるため統合しない。ただし、両者とも `pure_eval::eval_binary_pure` を共有する。

---

## 10. 変更ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `src/base/mod.rs` | `pub mod constexpr_eval;` 追加 |
| `src/base/constexpr_eval.rs` | 新規: ConstexprEnv, eval_constexpr_expr, eval_constexpr_block |
| `src/tree_parser/statement/mod.rs` | `parse_constexpr_declarations` にブロック形式パース追加 |
| `src/semantic_analyzer/mod.rs` | `evaluate_constexpr_by_name` で Block 検出時にブロック評価器を呼び出す |
| `resources/tests/` | constexpr ブロックのテストケース追加 |
| `docs/spec.md` | constexpr セクションにブロック形式の構文・仕様を追記 |
| `docs/grammar.bnf` | constexpr ブロック形式の文法を追記 |
