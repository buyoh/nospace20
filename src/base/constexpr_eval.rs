//! # constexpr ブロック評価器
//!
//! コンパイル時定数ブロック `constexpr: NAME { ... };` の評価を行うモジュール。
//!
//! `pure_eval.rs` と同様に、コンパイル時評価は semantic_analyzer・optimizer・interpreter
//! いずれにも属さない汎用機能として `base/` に配置する。

use std::collections::BTreeMap;

use crate::{
    base::{pure_eval, CodeParseError},
    code_parse_error,
    tree_parser::{Expression, LocatedExpression, LocatedStatement, Operator1, Operator2, Statement},
};

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
    ///
    /// 戻り値: 変数が見つかって代入できた場合 `true`、見つからなかった場合 `false`
    fn assign_variable(&mut self, name: &str, value: i64) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(v) = scope.get_mut(name) {
                *v = value;
                return true;
            }
        }
        false
    }
}

/// constexpr 環境内で式を評価する
///
/// 既存の `evaluate_constexpr_expr()` と異なり、ローカル変数を含む環境上で動作する。
/// `evaluate_constexpr_expr()` は raw/resolved/evaluating の3テーブルを使うが、
/// この関数は事前解決済みの `ConstexprEnv` 上で動作する。
pub fn eval_constexpr_expr(
    expr: &LocatedExpression,
    env: &mut ConstexprEnv,
) -> Result<i64, Vec<CodeParseError>> {
    let loc = expr.location.start;
    match &expr.expression {
        Expression::Factor(n) => Ok(*n),

        Expression::Variable(name) => env.get_variable(name).ok_or_else(|| {
            vec![code_parse_error!(
                loc,
                format!("'{}' is not defined in constexpr block", name)
            )]
        }),

        Expression::Operation1(op, inner) => {
            let v = eval_constexpr_expr(inner, env)?;
            match op {
                Operator1::Negative => Ok(v.wrapping_neg()),
                Operator1::LogicalNot => Ok(pure_eval::bool_to_int(v == 0)),
                _ => Err(vec![code_parse_error!(
                    loc,
                    "Ref/Deref is not allowed in constexpr block"
                )]),
            }
        }

        Expression::Operation2(op, l, r) => match op {
            // 代入演算は式としてではなく Statement 経由で処理する
            Operator2::Assign
            | Operator2::PlusAssign
            | Operator2::MinusAssign
            | Operator2::MultiplyAssign
            | Operator2::DivideAssign
            | Operator2::ModuloAssign => Err(vec![code_parse_error!(
                loc,
                "assignment expression is not supported in constexpr block"
            )]),
            // 短絡評価
            Operator2::LogicalAnd => {
                let lv = eval_constexpr_expr(l, env)?;
                if lv == 0 {
                    return Ok(0);
                }
                let rv = eval_constexpr_expr(r, env)?;
                Ok(pure_eval::bool_to_int(rv != 0))
            }
            Operator2::LogicalOr => {
                let lv = eval_constexpr_expr(l, env)?;
                if lv != 0 {
                    return Ok(1);
                }
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
        },

        Expression::If(cond, then_body, else_body) => {
            eval_constexpr_if(cond, then_body, else_body, env)
        }

        Expression::Block(stmts) => eval_constexpr_block(stmts, env),

        _ => Err(vec![code_parse_error!(
            loc,
            "expression is not compile-time evaluable in constexpr block"
        )]),
    }
}

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
                        stmt.location.start,
                        "static variables are not allowed in constexpr block"
                    )]);
                }
                if *is_final {
                    return Err(vec![code_parse_error!(
                        stmt.location.start,
                        "final variables are not allowed in constexpr block"
                    )]);
                }
                if array_size.is_some() {
                    return Err(vec![code_parse_error!(
                        stmt.location.start,
                        "arrays are not allowed in constexpr block"
                    )]);
                }
                // parse_variable_init では init_expr を `name = rhs` の形式で生成するため、
                // Assign 式の RHS を取り出して評価する
                let rhs = match &init.expression {
                    Expression::Operation2(Operator2::Assign, _lhs, rhs) => rhs.as_ref(),
                    _ => init.as_ref(),
                };
                let value = eval_constexpr_expr(rhs, env)?;
                env.declare_variable(name.clone(), value);
                last_value = Some(value);
            }

            Statement::Expression(expr) => {
                // 代入文（式文として現れる `a = b;` の形）を先にチェック
                if let Expression::Operation2(op, lhs, rhs) = &expr.expression {
                    if matches!(
                        op,
                        Operator2::Assign
                            | Operator2::PlusAssign
                            | Operator2::MinusAssign
                            | Operator2::MultiplyAssign
                            | Operator2::DivideAssign
                            | Operator2::ModuloAssign
                    ) {
                        let rhs_value = eval_constexpr_expr(rhs, env)?;
                        let new_value =
                            eval_constexpr_assign(op, lhs, rhs_value, env, stmt.location.start)?;
                        last_value = Some(new_value);
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

/// constexpr ブロック内での代入を処理する
///
/// NOTE: 代入式は tree_parser では `Expression::Operation2(Assign, lhs, rhs)` として解析される。
/// `eval_constexpr_expr` では代入を拒否するが、`Statement::Expression` として現れた場合には
/// この関数で処理する。
///
/// 戻り値: 代入後の変数の値
fn eval_constexpr_assign(
    op: &Operator2,
    target: &LocatedExpression,
    rhs_value: i64,
    env: &mut ConstexprEnv,
    loc: usize,
) -> Result<i64, Vec<CodeParseError>> {
    let name = match &target.expression {
        Expression::Variable(name) => name,
        _ => {
            return Err(vec![code_parse_error!(
                loc,
                "invalid assignment target in constexpr block"
            )])
        }
    };

    // constexpr テーブルの値への代入は禁止
    if env.constexpr_table.contains_key(name.as_str()) {
        return Err(vec![code_parse_error!(
            loc,
            format!("cannot assign to constexpr constant '{}'", name)
        )]);
    }

    let new_value = match op {
        Operator2::Assign => rhs_value,
        Operator2::PlusAssign
        | Operator2::MinusAssign
        | Operator2::MultiplyAssign
        | Operator2::DivideAssign
        | Operator2::ModuloAssign => {
            let old = env.get_variable(name).ok_or_else(|| {
                vec![code_parse_error!(
                    loc,
                    format!("'{}' is not defined", name)
                )]
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
            loc,
            format!("'{}' is not defined in constexpr block", name)
        )]);
    }
    Ok(new_value)
}

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
        Ok(0) // else なしの if: 偽のとき 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree_parser::{LocatedExpression, LocatedStatement};
    use crate::base::SourceLocation;

    fn dummy_loc() -> SourceLocation {
        SourceLocation::from_single(0)
    }

    fn make_factor(n: i64) -> Box<LocatedExpression> {
        Box::new(LocatedExpression {
            expression: Expression::Factor(n),
            location: dummy_loc(),
        })
    }

    fn make_variable(name: &str) -> Box<LocatedExpression> {
        Box::new(LocatedExpression {
            expression: Expression::Variable(name.to_string()),
            location: dummy_loc(),
        })
    }

    fn make_op2(op: Operator2, l: Box<LocatedExpression>, r: Box<LocatedExpression>) -> Box<LocatedExpression> {
        Box::new(LocatedExpression {
            expression: Expression::Operation2(op, l, r),
            location: dummy_loc(),
        })
    }

    fn empty_table() -> BTreeMap<String, i64> {
        BTreeMap::new()
    }

    #[test]
    fn test_eval_expr_factor() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let expr = make_factor(42);
        assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 42);
    }

    #[test]
    fn test_eval_expr_arithmetic() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        // 3 + 4 = 7
        let expr = make_op2(Operator2::Plus, make_factor(3), make_factor(4));
        assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 7);
        // 10 * 2 = 20
        let expr2 = make_op2(Operator2::Multiply, make_factor(10), make_factor(2));
        assert_eq!(eval_constexpr_expr(&expr2, &mut env).unwrap(), 20);
        // 10 / 0 => error
        let expr3 = make_op2(Operator2::Divide, make_factor(10), make_factor(0));
        assert!(eval_constexpr_expr(&expr3, &mut env).is_err());
    }

    #[test]
    fn test_eval_expr_variable() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        env.declare_variable("x".to_string(), 99);
        let expr = make_variable("x");
        assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 99);
    }

    #[test]
    fn test_eval_expr_constexpr_ref() {
        let mut table = BTreeMap::new();
        table.insert("CONST".to_string(), 123i64);
        let mut env = ConstexprEnv::new(&table);
        let expr = make_variable("CONST");
        assert_eq!(eval_constexpr_expr(&expr, &mut env).unwrap(), 123);
    }

    #[test]
    fn test_eval_expr_undefined_variable() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let expr = make_variable("undefined");
        assert!(eval_constexpr_expr(&expr, &mut env).is_err());
    }

    fn make_let_stmt(name: &str, value: i64) -> LocatedStatement {
        LocatedStatement {
            statement: Statement::VariableDeclaration(
                name.to_string(),
                make_factor(value),
                false,
                false,
                None,
            ),
            location: dummy_loc(),
        }
    }

    fn make_expr_stmt(expr: Box<LocatedExpression>) -> LocatedStatement {
        LocatedStatement {
            statement: Statement::Expression(expr),
            location: dummy_loc(),
        }
    }

    #[test]
    fn test_eval_block_let() {
        // { let: x(5); x; }  => 5
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let stmts = vec![
            make_let_stmt("x", 5),
            make_expr_stmt(make_variable("x")),
        ];
        assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 5);
    }

    #[test]
    fn test_eval_block_assign() {
        // { let: x(1); x = 10; x; } => 10
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let assign_expr = make_op2(Operator2::Assign, make_variable("x"), make_factor(10));
        let stmts = vec![
            make_let_stmt("x", 1),
            make_expr_stmt(assign_expr),
            make_expr_stmt(make_variable("x")),
        ];
        assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 10);
    }

    #[test]
    fn test_eval_block_compound_assign() {
        // { let: x(3); x += 7; x; } => 10
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let plus_assign = make_op2(Operator2::PlusAssign, make_variable("x"), make_factor(7));
        let stmts = vec![
            make_let_stmt("x", 3),
            make_expr_stmt(plus_assign),
            make_expr_stmt(make_variable("x")),
        ];
        assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 10);
    }

    #[test]
    fn test_eval_block_if() {
        // if 式: 条件真
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        // if(1) { 42; } else { 0; } => 42
        let then_stmts = vec![make_expr_stmt(make_factor(42))];
        let else_stmts = vec![make_expr_stmt(make_factor(0))];
        let if_expr = Box::new(LocatedExpression {
            expression: Expression::If(make_factor(1), then_stmts.clone(), else_stmts.clone()),
            location: dummy_loc(),
        });
        let stmts = vec![make_expr_stmt(if_expr)];
        assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 42);

        // 条件偽
        let mut env2 = ConstexprEnv::new(&table);
        let if_expr2 = Box::new(LocatedExpression {
            expression: Expression::If(make_factor(0), then_stmts, else_stmts),
            location: dummy_loc(),
        });
        let stmts2 = vec![make_expr_stmt(if_expr2)];
        assert_eq!(eval_constexpr_block(&stmts2, &mut env2).unwrap(), 0);
    }

    #[test]
    fn test_eval_block_nested_scope() {
        // { let: x(1); { let: x(2); x; }; x; }  => 外のxは1のまま
        // ネストしたBlockは式として評価
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let inner_stmts = vec![
            make_let_stmt("x", 2),
            make_expr_stmt(make_variable("x")),
        ];
        let block_expr = Box::new(LocatedExpression {
            expression: Expression::Block(inner_stmts),
            location: dummy_loc(),
        });
        let stmts = vec![
            make_let_stmt("x", 1),
            make_expr_stmt(block_expr),
            make_expr_stmt(make_variable("x")),
        ];
        // 最後の式は x = 1
        assert_eq!(eval_constexpr_block(&stmts, &mut env).unwrap(), 1);
    }

    #[test]
    fn test_eval_block_no_value_error() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        // 空のブロックはエラー
        assert!(eval_constexpr_block(&[], &mut env).is_err());
    }

    #[test]
    fn test_eval_block_static_error() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let stmts = vec![LocatedStatement {
            statement: Statement::VariableDeclaration(
                "x".to_string(),
                make_factor(1),
                true, // is_static=true
                false,
                None,
            ),
            location: dummy_loc(),
        }];
        assert!(eval_constexpr_block(&stmts, &mut env).is_err());
    }

    #[test]
    fn test_eval_block_final_error() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let stmts = vec![LocatedStatement {
            statement: Statement::VariableDeclaration(
                "x".to_string(),
                make_factor(1),
                false,
                true, // is_final=true
                None,
            ),
            location: dummy_loc(),
        }];
        assert!(eval_constexpr_block(&stmts, &mut env).is_err());
    }

    #[test]
    fn test_eval_block_array_error() {
        let table = empty_table();
        let mut env = ConstexprEnv::new(&table);
        let stmts = vec![LocatedStatement {
            statement: Statement::VariableDeclaration(
                "arr".to_string(),
                make_factor(0),
                false,
                false,
                Some(10), // array_size=Some(10)
            ),
            location: dummy_loc(),
        }];
        assert!(eval_constexpr_block(&stmts, &mut env).is_err());
    }

    #[test]
    fn test_eval_assign_to_constexpr_table_error() {
        // constexpr テーブルの定数への代入はエラー
        let mut table = BTreeMap::new();
        table.insert("CONST".to_string(), 42i64);
        let mut env = ConstexprEnv::new(&table);
        let assign = make_op2(Operator2::Assign, make_variable("CONST"), make_factor(1));
        let stmts = vec![make_expr_stmt(assign)];
        assert!(eval_constexpr_block(&stmts, &mut env).is_err());
    }
}
