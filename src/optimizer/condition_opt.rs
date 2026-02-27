//! # Condition Optimization Pass
//!
//! `If` / `While` の条件式を解析し、`ConditionMode` を `Zero` / `Negative` に切り替えることで
//! Whitespace の `JumpIfZero` / `JumpIfNegative` を直接活用する最適化パス。
//!
//! ## 変換パターン
//!
//! | 条件式 | 変換先 |
//! |---|---|
//! | `expr == 0` | `If(Zero, expr, ...)` |
//! | `expr != 0` | `If(Zero, expr, ...)` (then/else 入れ替え) |
//! | `expr < 0` | `If(Negative, expr, ...)` |
//! | `expr >= 0` | `If(Negative, expr, ...)` (then/else 入れ替え) |
//! | `expr1 == expr2` | `If(Zero, expr1 - expr2, ...)` |
//! | `expr1 != expr2` | `If(Zero, expr1 - expr2, ...)` (then/else 入れ替え) |
//! | `expr1 < expr2` | `If(Negative, expr1 - expr2, ...)` |
//! | `expr1 >= expr2` | `If(Negative, expr1 - expr2, ...)` (then/else 入れ替え) |
//! | `expr1 > expr2` | `If(Negative, expr2 - expr1, ...)` |
//! | `expr1 <= expr2` | `If(Negative, expr2 - expr1, ...)` (then/else 入れ替え) |
//!
//! `While` も同様のパターンで変換する。
//!
//! ## 注意事項
//!
//! - `LogicalAnd` / `LogicalOr` との組み合わせは対象外
//! - 最適化なしの結果を変化させない（セマンティクス保持）

use crate::base::SourceLocation;
use crate::semantic_analyzer::{
    Block, ConditionMode, ExecExpression, ExecStatement, LocatedExecExpression,
    LocatedExecStatement, Scope,
};
use crate::tree_parser::Operator2;

/// 条件式最適化パスを適用する
pub fn apply(scope: &mut Scope) {
    for func in &mut scope.functions {
        optimize_block(&mut func.block);
    }
    for stmt in &mut scope.root_statements {
        optimize_statement(stmt);
    }
    for stmt in &mut scope.static_init_statements {
        optimize_statement(stmt);
    }
}

fn optimize_block(block: &mut Block) {
    for stmt in &mut block.statements {
        optimize_statement(stmt);
    }
}

fn optimize_statement(stmt: &mut LocatedExecStatement) {
    match &mut stmt.statement {
        ExecStatement::Expression(expr) => optimize_located_expr(expr),
        ExecStatement::Return(Some(expr)) => optimize_located_expr(expr),
        ExecStatement::While(ref mut mode, ref mut cond, ref mut body) => {
            // 子ノードを先に再帰最適化
            optimize_located_expr(cond);
            optimize_block(body);

            // NonZero の場合のみパターン変換を試みる
            if *mode == ConditionMode::NonZero {
                let cond_loc = cond.location.clone();
                let cond_expr = std::mem::replace(&mut cond.expression, ExecExpression::Factor(0));
                let (new_mode, new_cond_expr) = optimize_while_nonzero(cond_expr, cond_loc);
                *mode = new_mode;
                cond.expression = new_cond_expr;
            }
        }
        ExecStatement::For(
            ref mut init,
            ref mut mode,
            ref mut cond,
            ref mut step,
            ref mut body,
        ) => {
            // 全ブロックを再帰最適化
            optimize_block(init);
            optimize_block(cond);
            optimize_block(step);
            optimize_block(body);

            // NonZero の場合のみ: cond ブロックの最後の式に対して ConditionMode 最適化を適用
            if *mode == ConditionMode::NonZero {
                if let Some(last_stmt) = cond.statements.last_mut() {
                    if let ExecStatement::Expression(ref mut located_expr) = last_stmt.statement {
                        let cond_loc = located_expr.location.clone();
                        let cond_expr = std::mem::replace(
                            &mut located_expr.expression,
                            ExecExpression::Factor(0),
                        );
                        let (new_mode, new_cond_expr) = optimize_while_nonzero(cond_expr, cond_loc);
                        *mode = new_mode;
                        located_expr.expression = new_cond_expr;
                    }
                }
            }
        }
        _ => {}
    }
}

/// `Box<LocatedExecExpression>` に対して最適化を適用する
///
/// std::mem::replace を使用して所有権を移動し、最適化後の値を戻す。
fn optimize_located_expr(located: &mut Box<LocatedExecExpression>) {
    let loc = located.location.clone();
    // 一時的に Factor(0) で置き換えて所有権を取り出す
    let expr = std::mem::replace(&mut located.expression, ExecExpression::Factor(0));
    located.expression = optimize_expression(expr, loc);
}

/// 式を最適化する（所有権を消費して変換後の式を返す）
fn optimize_expression(expr: ExecExpression, loc: SourceLocation) -> ExecExpression {
    match expr {
        // If(NonZero, ...) のみパターン変換を試みる
        ExecExpression::If(ConditionMode::NonZero, mut cond, mut then_block, mut else_block) => {
            // 子ノードを先に再帰最適化
            optimize_located_expr(&mut cond);
            optimize_block(&mut then_block);
            optimize_block(&mut else_block);

            // 条件式を分解してパターンマッチ
            let LocatedExecExpression {
                expression: cond_expr,
                location: cond_loc,
            } = *cond;
            optimize_if_nonzero(cond_expr, cond_loc, then_block, else_block, loc)
        }

        // 既に NonZero 以外の ConditionMode は子ノードのみ再帰最適化
        ExecExpression::If(mode, mut cond, mut then_block, mut else_block) => {
            optimize_located_expr(&mut cond);
            optimize_block(&mut then_block);
            optimize_block(&mut else_block);
            ExecExpression::If(mode, cond, then_block, else_block)
        }

        // While(NonZero, ...) は文レベルで処理されるため、式レベルでは処理不要
        // ブロック式
        ExecExpression::Block(mut block) => {
            optimize_block(&mut block);
            ExecExpression::Block(block)
        }

        // 単項演算
        ExecExpression::Operation1(op, mut inner) => {
            optimize_located_expr(&mut inner);
            ExecExpression::Operation1(op, inner)
        }

        // 二項演算
        ExecExpression::Operation2(op, mut left, mut right) => {
            optimize_located_expr(&mut left);
            optimize_located_expr(&mut right);
            ExecExpression::Operation2(op, left, right)
        }

        // 組み込み関数呼び出し
        ExecExpression::BuiltinFunction(kind, mut args) => {
            for arg in &mut args {
                optimize_located_expr(arg);
            }
            ExecExpression::BuiltinFunction(kind, args)
        }

        // ユーザー定義関数呼び出し
        ExecExpression::UserFunction(func_ref, mut args) => {
            for arg in &mut args {
                optimize_located_expr(arg);
            }
            ExecExpression::UserFunction(func_ref, args)
        }

        // リーフノード（Factor, Variable, ArrayAccess, InternalBuiltinFunction）
        other => other,
    }
}

/// `If(NonZero, cond_expr, then_block, else_block)` のパターンマッチによる最適化
fn optimize_if_nonzero(
    cond_expr: ExecExpression,
    cond_loc: SourceLocation,
    then_block: Block,
    else_block: Block,
    loc: SourceLocation,
) -> ExecExpression {
    match cond_expr {
        // expr == 0 → If(Zero, expr, then, else)
        // expr1 == expr2 → If(Zero, expr1 - expr2, then, else)
        ExecExpression::Operation2(Operator2::Equal, lhs, rhs) => {
            if is_zero_factor(&rhs) {
                ExecExpression::If(ConditionMode::Zero, lhs, then_block, else_block)
            } else if is_zero_factor(&lhs) {
                ExecExpression::If(ConditionMode::Zero, rhs, then_block, else_block)
            } else {
                let sub = wrap_expr(make_sub(lhs, rhs), loc);
                ExecExpression::If(ConditionMode::Zero, sub, then_block, else_block)
            }
        }

        // expr != 0 → If(Zero, expr, else, then)  ← then/else を入れ替え
        // expr1 != expr2 → If(Zero, expr1 - expr2, else, then)
        ExecExpression::Operation2(Operator2::NotEqual, lhs, rhs) => {
            if is_zero_factor(&rhs) {
                ExecExpression::If(ConditionMode::Zero, lhs, else_block, then_block)
            } else if is_zero_factor(&lhs) {
                ExecExpression::If(ConditionMode::Zero, rhs, else_block, then_block)
            } else {
                let sub = wrap_expr(make_sub(lhs, rhs), loc);
                ExecExpression::If(ConditionMode::Zero, sub, else_block, then_block)
            }
        }

        // expr < 0 → If(Negative, expr, then, else)
        // expr1 < expr2 → If(Negative, expr1 - expr2, then, else)
        ExecExpression::Operation2(Operator2::Less, lhs, rhs) => {
            if is_zero_factor(&rhs) {
                ExecExpression::If(ConditionMode::Negative, lhs, then_block, else_block)
            } else {
                let sub = wrap_expr(make_sub(lhs, rhs), loc);
                ExecExpression::If(ConditionMode::Negative, sub, then_block, else_block)
            }
        }

        // expr >= 0 → If(Negative, expr, else, then)  ← then/else を入れ替え
        // expr1 >= expr2 → If(Negative, expr1 - expr2, else, then)
        ExecExpression::Operation2(Operator2::GreaterEqual, lhs, rhs) => {
            if is_zero_factor(&rhs) {
                ExecExpression::If(ConditionMode::Negative, lhs, else_block, then_block)
            } else {
                let sub = wrap_expr(make_sub(lhs, rhs), loc);
                ExecExpression::If(ConditionMode::Negative, sub, else_block, then_block)
            }
        }

        // expr1 > expr2 → expr2 - expr1 < 0 → If(Negative, expr2 - expr1, then, else)
        ExecExpression::Operation2(Operator2::Greater, lhs, rhs) => {
            let sub = wrap_expr(make_sub(rhs, lhs), loc);
            ExecExpression::If(ConditionMode::Negative, sub, then_block, else_block)
        }

        // expr1 <= expr2 → expr2 - expr1 >= 0 → If(Negative, expr2 - expr1, else, then)
        ExecExpression::Operation2(Operator2::LessEqual, lhs, rhs) => {
            let sub = wrap_expr(make_sub(rhs, lhs), loc);
            ExecExpression::If(ConditionMode::Negative, sub, else_block, then_block)
        }

        // パターンに一致しない場合はそのまま返す
        other => {
            let cond = wrap_expr(other, cond_loc);
            ExecExpression::If(ConditionMode::NonZero, cond, then_block, else_block)
        }
    }
}

/// `While(NonZero, cond_expr, body)` のパターンマッチによる最適化
///
/// 戻り値: (最適化された ConditionMode, 最適化された条件式)
fn optimize_while_nonzero(
    cond_expr: ExecExpression,
    cond_loc: SourceLocation,
) -> (ConditionMode, ExecExpression) {
    match cond_expr {
        // expr != 0 → While(NonZero, expr, body)  ← 条件式を単純化（COMPARATOR 不要）
        // expr1 != expr2 → While(NonZero, expr1 - expr2, body)
        ExecExpression::Operation2(Operator2::NotEqual, lhs, rhs) => {
            if is_zero_factor(&rhs) {
                (ConditionMode::NonZero, lhs.expression)
            } else if is_zero_factor(&lhs) {
                (ConditionMode::NonZero, rhs.expression)
            } else {
                let sub = make_sub(lhs, rhs);
                (ConditionMode::NonZero, wrap_expr(sub, cond_loc).expression)
            }
        }

        // expr == 0 → While(Zero, expr, body)
        // expr1 == expr2 → While(Zero, expr1 - expr2, body)
        ExecExpression::Operation2(Operator2::Equal, lhs, rhs) => {
            if is_zero_factor(&rhs) {
                (ConditionMode::Zero, lhs.expression)
            } else if is_zero_factor(&lhs) {
                (ConditionMode::Zero, rhs.expression)
            } else {
                let sub = make_sub(lhs, rhs);
                (ConditionMode::Zero, wrap_expr(sub, cond_loc).expression)
            }
        }

        // expr < 0 → While(Negative, expr, body)
        // expr1 < expr2 → While(Negative, expr1 - expr2, body)
        ExecExpression::Operation2(Operator2::Less, lhs, rhs) => {
            if is_zero_factor(&rhs) {
                (ConditionMode::Negative, lhs.expression)
            } else {
                let sub = make_sub(lhs, rhs);
                (ConditionMode::Negative, wrap_expr(sub, cond_loc).expression)
            }
        }

        // パターンに一致しない場合はそのまま返す
        other => (
            ConditionMode::NonZero,
            wrap_expr(other, cond_loc).expression,
        ),
    }
}

/// 式が `Factor(0)` かどうかを判定する
fn is_zero_factor(expr: &Box<LocatedExecExpression>) -> bool {
    matches!(expr.expression, ExecExpression::Factor(0))
}

/// `lhs - rhs` の Operation2(Minus, ...) を生成する
fn make_sub(lhs: Box<LocatedExecExpression>, rhs: Box<LocatedExecExpression>) -> ExecExpression {
    ExecExpression::Operation2(Operator2::Minus, lhs, rhs)
}

/// ExecExpression を LocatedExecExpression の Box で包む（位置情報はオプティマイザ生成）
fn wrap_expr(expr: ExecExpression, loc: SourceLocation) -> Box<LocatedExecExpression> {
    Box::new(LocatedExecExpression {
        expression: expr,
        location: loc,
    })
}
