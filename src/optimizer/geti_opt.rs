//! # Geti/Getc Optimization Pass
//!
//! `p = __geti()` / `p = __getc()` パターンを検出し、一時領域 (`TEMP_PTR`) を経由せずに
//! 変数アドレスへ直接入力する `InternalBuiltinFunction(Getiv/Getcv)` に変換する最適化パス。
//!
//! ## 変換パターン
//!
//! ```text
//! ExecStatement::Expression(
//!     Operation2(Assign,
//!         Variable(var_ref),
//!         BuiltinFunction(Geti, [])
//!     )
//! )
//! → ExecStatement::Expression(InternalBuiltinFunction(Getiv(var_ref)))
//! ```
//!
//! ## 適用条件
//!
//! - 左辺が単純な変数参照 (`Variable(IdentifierRef)`) であること
//! - 右辺が引数なしの `__geti()` または `__getc()` であること
//! - 文の直接の式 (`ExecStatement::Expression`) でのみ適用
//!
//! ## 命令数削減効果
//!
//! | パターン | 変数種別 | 最適化前 | 最適化後 | 削減 |
//! |---|---|---|---|---|
//! | `p = __geti()` | グローバル | 9 命令 | 4 命令 | 5 |
//! | `p = __geti()` | ローカル | 13 命令 | 7 命令 | 6 |

use crate::base::SourceLocation;
use crate::semantic_analyzer::{
    Block, BuiltinFunctionKind, ExecExpression, ExecStatement, InternalBuiltinFunctionKind,
    LocatedExecExpression, LocatedExecStatement, Scope,
};
use crate::tree_parser::Operator2;

/// geti/getc 最適化パスを適用する
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
        ExecStatement::Expression(expr) => {
            // パターンマッチを試みる
            if let Some(new_expr) = try_transform_geti(expr) {
                expr.expression = new_expr;
            } else {
                // パターンに一致しなかった場合は再帰的に子ノードを処理
                recurse_into_expr(expr);
            }
        }
        ExecStatement::Return(Some(expr)) => {
            recurse_into_expr(expr);
        }
        _ => {}
    }
}

/// `Operation2(Assign, Variable(var_ref), BuiltinFunction(Geti/Getc, []))` を
/// `InternalBuiltinFunction(Getiv/Getcv(var_ref))` に変換する。
///
/// 変換できた場合は `Some(new_inner_expression)` を返す。
fn try_transform_geti(
    located: &LocatedExecExpression,
) -> Option<ExecExpression> {
    if let ExecExpression::Operation2(Operator2::Assign, lhs, rhs) = &located.expression {
        if let ExecExpression::Variable(var_ref) = &lhs.expression {
            match &rhs.expression {
                ExecExpression::BuiltinFunction(BuiltinFunctionKind::Geti, args) if args.is_empty() => {
                    Some(ExecExpression::InternalBuiltinFunction(
                        InternalBuiltinFunctionKind::Getiv(*var_ref),
                    ))
                }
                ExecExpression::BuiltinFunction(BuiltinFunctionKind::Getc, args) if args.is_empty() => {
                    Some(ExecExpression::InternalBuiltinFunction(
                        InternalBuiltinFunctionKind::Getcv(*var_ref),
                    ))
                }
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// 式の子ノードを再帰的に処理する（ブロック式・If・While など）
fn recurse_into_expr(located: &mut Box<LocatedExecExpression>) {
    match &mut located.expression {
        ExecExpression::If(_, cond, then_block, else_block) => {
            recurse_into_expr(cond);
            optimize_block(then_block);
            optimize_block(else_block);
        }
        ExecExpression::While(_, cond, body) => {
            recurse_into_expr(cond);
            optimize_block(body);
        }
        ExecExpression::Block(block) => {
            optimize_block(block);
        }
        ExecExpression::Operation1(_, inner) => {
            recurse_into_expr(inner);
        }
        ExecExpression::Operation2(_, left, right) => {
            recurse_into_expr(left);
            recurse_into_expr(right);
        }
        ExecExpression::BuiltinFunction(_, args) | ExecExpression::UserFunction(_, args) => {
            for arg in args {
                recurse_into_expr(arg);
            }
        }
        _ => {}
    }
}

/// geti 最適化で使用するダミー位置情報（オプティマイザ生成ノード用）
#[allow(dead_code)]
fn dummy_location() -> SourceLocation {
    SourceLocation::new(0, 0)
}
