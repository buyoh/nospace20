//! # Constant Folding Pass
//!
//! コンパイル時に評価可能な定数式を事前に計算し、`Factor(値)` に置換する。
//!
//! ## 変換パターン
//!
//! ### 算術演算
//! ```text
//! Operation2(Plus, Factor(a), Factor(b))     → Factor(a + b)
//! Operation2(Minus, Factor(a), Factor(b))    → Factor(a - b)
//! Operation2(Multiply, Factor(a), Factor(b)) → Factor(a * b)
//! Operation2(Divide, Factor(a), Factor(b))   → Factor(a / b)  ※ b != 0
//! Operation2(Modulo, Factor(a), Factor(b))   → Factor(a % b)  ※ b != 0
//! ```
//!
//! ### 単項演算
//! ```text
//! Operation1(Negative, Factor(a))    → Factor(-a)
//! Operation1(LogicalNot, Factor(a))  → Factor(if a == 0 { 1 } else { 0 })
//! ```
//!
//! ### 定数条件の if/while
//! 条件式が定数に畳み込まれた場合、ConditionMode に基づいて真偽を評価し、
//! 対応するブロックに置換する。

use crate::base::{pure_eval, SourceLocation};
use crate::semantic_analyzer::{
    Block, ConditionMode, ExecExpression, ExecStatement, LocatedExecExpression,
    LocatedExecStatement, Scope,
};
use crate::tree_parser::{Operator1, Operator2};

/// 定数畳み込みパスを適用する
pub fn apply(scope: &mut Scope) {
    for func in &mut scope.functions {
        if func.is_unused() {
            continue;
        }
        fold_block(&mut func.block);
    }
    for stmt in &mut scope.root_statements {
        fold_statement(stmt);
    }
    for stmt in &mut scope.static_init_statements {
        fold_statement(stmt);
    }
}

fn fold_block(block: &mut Block) {
    for stmt in &mut block.statements {
        fold_statement(stmt);
    }
    // ネストしたスコープの static_init も畳み込む
    for stmt in &mut block.scope.static_init_statements {
        fold_statement(stmt);
    }
}

fn fold_statement(stmt: &mut LocatedExecStatement) {
    match &mut stmt.statement {
        ExecStatement::Expression(expr) => fold_located_expr(expr),
        ExecStatement::Return(Some(expr)) => fold_located_expr(expr),
        ExecStatement::While(ref mut mode, ref mut cond, ref mut body) => {
            fold_located_expr(cond);
            fold_block(body);
            // 定数条件の while は文レベルで処理
            if let ExecExpression::Factor(v) = cond.expression {
                let runs = match *mode {
                    ConditionMode::NonZero => v != 0,
                    ConditionMode::Zero => v == 0,
                    ConditionMode::Negative => v < 0,
                };
                if !runs {
                    body.statements.clear();
                }
            }
        }
        ExecStatement::For(
            ref mut init,
            ref mut mode,
            ref mut cond,
            ref mut step,
            ref mut body,
        ) => {
            fold_block(init);
            fold_block(cond);
            fold_block(step);
            fold_block(body);
            // 定数条件の for: cond ブロックの最後の文が Factor(定数) なら評価可能
            if let Some(last_stmt) = cond.statements.last() {
                if let ExecStatement::Expression(located_expr) = &last_stmt.statement {
                    if let ExecExpression::Factor(v) = located_expr.expression {
                        let runs = match *mode {
                            ConditionMode::NonZero => v != 0,
                            ConditionMode::Zero => v == 0,
                            ConditionMode::Negative => v < 0,
                        };
                        if !runs {
                            // ループが実行されない → body をクリア
                            body.statements.clear();
                            step.statements.clear();
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// `Box<LocatedExecExpression>` に対して定数畳み込みを適用する
///
/// std::mem::replace を使用して所有権を移動し、最適化後の値を戻す。
fn fold_located_expr(located: &mut Box<LocatedExecExpression>) {
    let loc = located.location.clone();
    let expr = std::mem::replace(&mut located.expression, ExecExpression::Factor(0));
    located.expression = fold_expression(expr, loc);
}

/// 式を定数畳み込みする（所有権を消費して変換後の式を返す）
///
/// ボトムアップで再帰的に畳み込む。
fn fold_expression(expr: ExecExpression, loc: SourceLocation) -> ExecExpression {
    match expr {
        // --- 二項演算 ---
        ExecExpression::Operation2(op, mut lhs, mut rhs) => {
            fold_located_expr(&mut lhs);
            fold_located_expr(&mut rhs);
            try_fold_op2(op, lhs, rhs, loc)
        }

        // --- 単項演算 ---
        ExecExpression::Operation1(op, mut operand) => {
            fold_located_expr(&mut operand);
            try_fold_op1(op, operand, loc)
        }

        // --- If 式 ---
        ExecExpression::If(mode, mut cond, mut then_block, mut else_block) => {
            fold_located_expr(&mut cond);
            fold_block(&mut then_block);
            fold_block(&mut else_block);
            try_fold_if(mode, cond, then_block, else_block)
        }

        // --- Block ---
        ExecExpression::Block(mut block) => {
            fold_block(&mut block);
            ExecExpression::Block(block)
        }

        // --- 関数呼び出し ---
        ExecExpression::BuiltinFunction(kind, mut args) => {
            for arg in &mut args {
                fold_located_expr(arg);
            }
            ExecExpression::BuiltinFunction(kind, args)
        }
        ExecExpression::UserFunction(id_ref, mut args) => {
            for arg in &mut args {
                fold_located_expr(arg);
            }
            ExecExpression::UserFunction(id_ref, args)
        }

        // --- 配列アクセス ---
        ExecExpression::ArrayAccess(id_ref, mut idx_expr, size) => {
            fold_located_expr(&mut idx_expr);
            ExecExpression::ArrayAccess(id_ref, idx_expr, size)
        }

        ExecExpression::TypeAssertion(mut inner, value_type) => {
            fold_located_expr(&mut inner);
            ExecExpression::TypeAssertion(inner, value_type)
        }
        ExecExpression::VoidCast(mut inner) => {
            fold_located_expr(&mut inner);
            ExecExpression::VoidCast(inner)
        }
        ExecExpression::StructFieldAccess(mut base, offset, array_size, field_type) => {
            fold_located_expr(&mut base);
            ExecExpression::StructFieldAccess(base, offset, array_size, field_type)
        }
        ExecExpression::StructFieldArrayAccess(mut base, offset, mut idx_expr, size) => {
            fold_located_expr(&mut base);
            fold_located_expr(&mut idx_expr);
            ExecExpression::StructFieldArrayAccess(base, offset, idx_expr, size)
        }

        // --- 末端ノード（変換なし） ---
        other => other,
    }
}

/// 二項演算の定数畳み込みを試みる
fn try_fold_op2(
    op: Operator2,
    lhs: Box<LocatedExecExpression>,
    rhs: Box<LocatedExecExpression>,
    _loc: SourceLocation,
) -> ExecExpression {
    // 両オペランドが定数の場合のみ畳み込む
    if let (ExecExpression::Factor(a), ExecExpression::Factor(b)) =
        (&lhs.expression, &rhs.expression)
    {
        let (a, b) = (*a, *b);
        let result = pure_eval::eval_binary_pure(&op, a, b);
        if let Some(v) = result {
            return ExecExpression::Factor(v);
        }
    }

    // 部分的な簡約（一方が定数 0 or 1 の場合）
    let lhs_val = if let ExecExpression::Factor(v) = lhs.expression {
        Some(v)
    } else {
        None
    };
    let rhs_val = if let ExecExpression::Factor(v) = rhs.expression {
        Some(v)
    } else {
        None
    };

    // 片方が定数の場合の簡約はオペランドを再構築して返す
    let lhs = Box::new(LocatedExecExpression {
        expression: if let Some(v) = lhs_val {
            ExecExpression::Factor(v)
        } else {
            lhs.expression
        },
        location: lhs.location,
    });
    let rhs = Box::new(LocatedExecExpression {
        expression: if let Some(v) = rhs_val {
            ExecExpression::Factor(v)
        } else {
            rhs.expression
        },
        location: rhs.location,
    });

    ExecExpression::Operation2(op, lhs, rhs)
}

/// 単項演算の定数畳み込みを試みる
fn try_fold_op1(
    op: Operator1,
    operand: Box<LocatedExecExpression>,
    _loc: SourceLocation,
) -> ExecExpression {
    if let ExecExpression::Factor(a) = operand.expression {
        let result = pure_eval::eval_unary_pure(&op, a);
        if let Some(v) = result {
            return ExecExpression::Factor(v);
        }
        // 展開した Factor を戻す
        let operand = Box::new(LocatedExecExpression {
            expression: ExecExpression::Factor(a),
            location: operand.location,
        });
        return ExecExpression::Operation1(op, operand);
    }
    ExecExpression::Operation1(op, operand)
}

/// If の定数条件畳み込みを試みる
///
/// 条件が定数に畳み込まれていた場合、ConditionMode に基づき真偽を判定し、
/// 対応するブロックに置換する。
fn try_fold_if(
    mode: ConditionMode,
    cond: Box<LocatedExecExpression>,
    then_block: Block,
    else_block: Block,
) -> ExecExpression {
    if let ExecExpression::Factor(v) = cond.expression {
        let take_then = match mode {
            ConditionMode::NonZero => v != 0,
            ConditionMode::Zero => v == 0,
            ConditionMode::Negative => v < 0,
        };
        return if take_then {
            ExecExpression::Block(then_block)
        } else {
            ExecExpression::Block(else_block)
        };
    }
    ExecExpression::If(mode, cond, then_block, else_block)
}
