//! # Return 解析モジュール
//!
//! 関数本体の return 文の存在チェックと、すべての制御パスでの return 保証チェックを担当する。
//!
//! これらの関数は副作用のない純粋関数であり、AST 型にのみ依存する。

use crate::tree_parser::{Expression, LocatedStatement, Statement};

/// 関数本体に return: 文が存在するか再帰的にチェックする
///
/// ネストした if/while/block の中もすべてチェックするが、ネストされた関数宣言の中は除外する
pub(super) fn has_return_statement(statements: &[LocatedStatement]) -> bool {
    for stat in statements {
        match &stat.statement {
            Statement::Return(Some(_)) => return true,
            Statement::Return(None) => {} // void return は int 返却とみなさない
            Statement::Expression(expr) => {
                if expr_contains_return(&expr.expression) {
                    return true;
                }
            }
            Statement::While(_, stmts) => {
                if has_return_statement(stmts) {
                    return true;
                }
            }
            Statement::For(init, cond, step, body) => {
                for block in &[init, cond, step, body] {
                    if has_return_statement(block) {
                        return true;
                    }
                }
            }
            // ネストした関数宣言は除外（別の関数の return なので）
                Statement::FunctionDeclaration(_, _, _, _) => {}
            _ => {}
        }
    }
    false
}

/// 式の中に return: 文が含まれるかチェックする。if/block 内の return: を再帰的にチェック
fn expr_contains_return(expr: &Expression) -> bool {
    match expr {
        Expression::If(_, then_stmts, else_stmts) => {
            has_return_statement(then_stmts) || has_return_statement(else_stmts)
        }
        Expression::Block(stmts) => has_return_statement(stmts),
        _ => false,
    }
}

/// 関数本体がすべての制御パスで return を保証するかチェックする
///
/// 軽量な到達可能性チェック（完全な制御フロー解析ではない）:
/// - 最後の文が Return → true
/// - 最後の文が if-else（else あり）かつ両ブランチが保証 → true
/// - それ以外 → false
pub(super) fn guarantees_return(statements: &[LocatedStatement]) -> bool {
    match statements.last() {
        None => false,
        Some(last) => match &last.statement {
            Statement::Return(Some(_)) => true,
            Statement::Return(None) => false, // void return は値の返却を保証しない
            Statement::Expression(expr) => expr_guarantees_return(&expr.expression),
            _ => false,
        },
    }
}

/// 式がすべての制御パスで return を保証するかチェックする
fn expr_guarantees_return(expr: &Expression) -> bool {
    match expr {
        Expression::If(_, then_stmts, else_stmts) => {
            // else なし（空の else_stmts）の if は保証しない
            if else_stmts.is_empty() {
                return false;
            }
            // 両方のブランチが保証する場合のみ保証
            guarantees_return(then_stmts) && guarantees_return(else_stmts)
        }
        Expression::Block(stmts) => guarantees_return(stmts),
        _ => false,
    }
}
