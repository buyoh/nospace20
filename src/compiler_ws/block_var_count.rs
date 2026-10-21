//! ブロック変数の数え上げ
//!
//! コード生成で `enter_block_scope` が消費するオフセットの合計を、生成前に求める。
//! 関数フレームのサイズと、グローバル領域（`global_layout`）の初期化文用領域の計算で共有する。
//!
//! バグ: 未チェックの `+` / `sum()` で、巨大な配列を持つブロックが並ぶと debug では panic、
//! release では wrap して、フレーム・グローバル領域を小さく見積もったコードを出力していた。
//! 解決策: すべて checked_add で合計し、usize に収まらない場合は None を返す（呼び出し元でエラーにする）。

use crate::semantic_analyzer::{Block, ExecStatement, LocatedExecStatement};

/// 関数内の全ブロック（ネスト含む）の変数合計数を計算（usize に収まらない場合は None）
pub(super) fn calculate_total_variable_count(block: &Block) -> Option<usize> {
    block
        .scope
        .variable_count
        .checked_add(count_nested_vars_in_statements(&block.statements)?)
}

/// 2 つの合計を checked_add で足す
fn add(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    a?.checked_add(b?)
}

/// 文の中のブロック（ネスト含む）の変数合計数を計算
///
/// 文が属するスコープ自身の変数は数えない（呼び出し側で別に配置済みのため）。
/// usize に収まらない場合は None。
pub(super) fn count_nested_vars_in_statements(stmts: &[LocatedExecStatement]) -> Option<usize> {
    stmts.iter().try_fold(0usize, |total, located| {
        total.checked_add(count_nested_vars_in_statement(&located.statement)?)
    })
}

fn count_nested_vars_in_statement(stmt: &ExecStatement) -> Option<usize> {
    match stmt {
        ExecStatement::Expression(located_expr) | ExecStatement::Return(Some(located_expr)) => {
            count_nested_vars_in_expression(located_expr)
        }
        ExecStatement::While(_mode, cond, body) => add(
            count_nested_vars_in_expression(cond),
            calculate_total_variable_count(body),
        ),
        ExecStatement::For(init, _mode, cond, step, body) => {
            // init は for スコープ自体（変数は init.scope.variable_count に含まれる）
            // init.statements の中に更にネストされた変数がある場合を処理
            [init, cond, step, body]
                .iter()
                .try_fold(0usize, |total, block| {
                    total.checked_add(calculate_total_variable_count(block)?)
                })
        }
        ExecStatement::Return(None) | ExecStatement::Break | ExecStatement::Continue => Some(0),
    }
}

fn count_nested_vars_in_expression(
    located_expr: &crate::semantic_analyzer::LocatedExecExpression,
) -> Option<usize> {
    use crate::semantic_analyzer::ExecExpression;
    match &located_expr.expression {
        ExecExpression::If(_mode, cond, then_block, else_block) => add(
            count_nested_vars_in_expression(cond),
            add(
                calculate_total_variable_count(then_block),
                calculate_total_variable_count(else_block),
            ),
        ),
        ExecExpression::Block(block) => calculate_total_variable_count(block),
        ExecExpression::Operation1(_, inner) => count_nested_vars_in_expression(inner),
        ExecExpression::Operation2(_, l, r) => add(
            count_nested_vars_in_expression(l),
            count_nested_vars_in_expression(r),
        ),
        ExecExpression::BuiltinFunction(_, args) | ExecExpression::UserFunction(_, args) => {
            args.iter().try_fold(0usize, |total, arg| {
                total.checked_add(count_nested_vars_in_expression(arg)?)
            })
        }
        ExecExpression::ArrayAccess(_, index_expr, _) => {
            count_nested_vars_in_expression(index_expr)
        }
        ExecExpression::Variable(_, _) | ExecExpression::Factor(_) => Some(0),
        ExecExpression::InternalBuiltinFunction(_) => Some(0),
        ExecExpression::TypeAssertion(inner, _) => count_nested_vars_in_expression(inner),
        ExecExpression::VoidCast(inner) => count_nested_vars_in_expression(inner),
        ExecExpression::StructFieldAccess(base, _, _, _) => count_nested_vars_in_expression(base),
        ExecExpression::StructFieldArrayAccess(base, _, index_expr, _) => add(
            count_nested_vars_in_expression(base),
            count_nested_vars_in_expression(index_expr),
        ),
    }
}
