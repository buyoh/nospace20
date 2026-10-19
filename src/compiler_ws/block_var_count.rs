//! ブロック変数の数え上げ
//!
//! コード生成で `enter_block_scope` が消費するオフセットの合計を、生成前に求める。
//! 関数フレームのサイズと、グローバル領域（`global_layout`）の初期化文用領域の計算で共有する。

use crate::semantic_analyzer::{Block, ExecStatement, LocatedExecStatement};

/// 関数内の全ブロック（ネスト含む）の変数合計数を計算
pub(super) fn calculate_total_variable_count(block: &Block) -> usize {
    block.scope.variable_count + count_nested_vars_in_statements(&block.statements)
}

/// 文の中のブロック（ネスト含む）の変数合計数を計算
///
/// 文が属するスコープ自身の変数は数えない（呼び出し側で別に配置済みのため）。
pub(super) fn count_nested_vars_in_statements(stmts: &[LocatedExecStatement]) -> usize {
    stmts
        .iter()
        .map(|located| count_nested_vars_in_statement(&located.statement))
        .sum()
}

fn count_nested_vars_in_statement(stmt: &ExecStatement) -> usize {
    match stmt {
        ExecStatement::Expression(located_expr) | ExecStatement::Return(Some(located_expr)) => {
            count_nested_vars_in_expression(located_expr)
        }
        ExecStatement::While(_mode, cond, body) => {
            count_nested_vars_in_expression(cond) + calculate_total_variable_count(body)
        }
        ExecStatement::For(init, _mode, cond, step, body) => {
            // init は for スコープ自体（変数は init.scope.variable_count に含まれる）
            // init.statements の中に更にネストされた変数がある場合を処理
            calculate_total_variable_count(init)
                + calculate_total_variable_count(cond)
                + calculate_total_variable_count(step)
                + calculate_total_variable_count(body)
        }
        ExecStatement::Return(None) | ExecStatement::Break | ExecStatement::Continue => 0,
    }
}

fn count_nested_vars_in_expression(
    located_expr: &crate::semantic_analyzer::LocatedExecExpression,
) -> usize {
    use crate::semantic_analyzer::ExecExpression;
    match &located_expr.expression {
        ExecExpression::If(_mode, cond, then_block, else_block) => {
            count_nested_vars_in_expression(cond)
                + calculate_total_variable_count(then_block)
                + calculate_total_variable_count(else_block)
        }
        ExecExpression::Block(block) => calculate_total_variable_count(block),
        ExecExpression::Operation1(_, inner) => count_nested_vars_in_expression(inner),
        ExecExpression::Operation2(_, l, r) => {
            count_nested_vars_in_expression(l) + count_nested_vars_in_expression(r)
        }
        ExecExpression::BuiltinFunction(_, args) | ExecExpression::UserFunction(_, args) => args
            .iter()
            .map(|arg| count_nested_vars_in_expression(arg))
            .sum(),
        ExecExpression::ArrayAccess(_, index_expr, _) => {
            count_nested_vars_in_expression(index_expr)
        }
        ExecExpression::Variable(_, _) | ExecExpression::Factor(_) => 0,
        ExecExpression::InternalBuiltinFunction(_) => 0,
        ExecExpression::TypeAssertion(inner, _) => count_nested_vars_in_expression(inner),
        ExecExpression::VoidCast(inner) => count_nested_vars_in_expression(inner),
        ExecExpression::StructFieldAccess(base, _, _, _) => count_nested_vars_in_expression(base),
        ExecExpression::StructFieldArrayAccess(base, _, index_expr, _) => {
            count_nested_vars_in_expression(base) + count_nested_vars_in_expression(index_expr)
        }
    }
}
