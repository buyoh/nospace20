//! # Dead Code Elimination Pass
//!
//! `main` 関数から到達不可能な関数を特定し、ダミー関数に置換する最適化パス。
//!
//! ## アルゴリズム
//!
//! 1. `main` 関数をルートとして到達可能集合を初期化
//! 2. BFS で呼び出しグラフを探索し、到達可能な関数を収集
//! 3. 到達不可能な関数を `Function::dummy()` に置換
//!
//! ## 注意事項
//!
//! - `main` 関数が存在しない場合は最適化をスキップ
//! - グローバル変数の初期化式・static 変数初期化式から呼ばれる関数も到達可能とする
//! - `Scope.functions` のインデックスは変えず、ダミー置換のみ行う（インデックスずれなし）

use std::collections::{HashSet, VecDeque};

use crate::semantic_analyzer::{
    Block, ExecExpression, ExecStatement, Function, LocatedExecStatement, Scope,
};

/// dead_code パスを適用する
pub fn apply(scope: &mut Scope) {
    let main_idx = match scope.main_function_index {
        Some(idx) => idx,
        None => return, // main がない場合はスキップ
    };

    // 到達可能な関数インデックスを BFS で収集
    let reachable = collect_reachable(scope, main_idx);

    // 到達不可能な関数をダミーに置換
    let total = scope.functions.len();
    for idx in 0..total {
        if !reachable.contains(&idx) {
            scope.functions[idx] = Function::dummy();
        }
    }
}

/// BFS で到達可能な関数インデックスの集合を返す
fn collect_reachable(scope: &Scope, main_idx: usize) -> HashSet<usize> {
    let mut reachable = HashSet::new();
    let mut worklist = VecDeque::new();

    reachable.insert(main_idx);
    worklist.push_back(main_idx);

    // ルートスコープの static_init_statements と root_statements からも収集
    for stmt in &scope.static_init_statements {
        collect_called_in_statement(stmt, &mut reachable, &mut worklist);
    }
    for stmt in &scope.root_statements {
        collect_called_in_statement(stmt, &mut reachable, &mut worklist);
    }

    while let Some(func_idx) = worklist.pop_front() {
        if func_idx >= scope.functions.len() {
            continue;
        }
        let func = &scope.functions[func_idx];
        collect_called_in_block(&func.block, &mut reachable, &mut worklist);
    }

    reachable
}

fn collect_called_in_block(
    block: &Block,
    reachable: &mut HashSet<usize>,
    worklist: &mut VecDeque<usize>,
) {
    // 関数ブロックの static_init_statements も走査
    for stmt in &block.scope.static_init_statements {
        collect_called_in_statement(stmt, reachable, worklist);
    }
    for stmt in &block.statements {
        collect_called_in_statement(stmt, reachable, worklist);
    }
}

fn collect_called_in_statement(
    stmt: &LocatedExecStatement,
    reachable: &mut HashSet<usize>,
    worklist: &mut VecDeque<usize>,
) {
    match &stmt.statement {
        ExecStatement::Expression(expr) => {
            collect_called_in_expr(&expr.expression, reachable, worklist)
        }
        ExecStatement::Return(Some(expr)) => {
            collect_called_in_expr(&expr.expression, reachable, worklist)
        }
        ExecStatement::While(_, cond, body) => {
            collect_called_in_expr(&cond.expression, reachable, worklist);
            collect_called_in_block(body, reachable, worklist);
        }
        ExecStatement::For(init, _, cond, step, body) => {
            collect_called_in_block(init, reachable, worklist);
            collect_called_in_block(cond, reachable, worklist);
            collect_called_in_block(step, reachable, worklist);
            collect_called_in_block(body, reachable, worklist);
        }
        _ => {}
    }
}

fn collect_called_in_expr(
    expr: &ExecExpression,
    reachable: &mut HashSet<usize>,
    worklist: &mut VecDeque<usize>,
) {
    match expr {
        ExecExpression::UserFunction(func_ref, args) => {
            let idx = func_ref.local_index;
            if reachable.insert(idx) {
                worklist.push_back(idx);
            }
            for arg in args {
                collect_called_in_expr(&arg.expression, reachable, worklist);
            }
        }
        ExecExpression::If(_, cond, then_block, else_block) => {
            collect_called_in_expr(&cond.expression, reachable, worklist);
            collect_called_in_block(then_block, reachable, worklist);
            collect_called_in_block(else_block, reachable, worklist);
        }
        ExecExpression::Block(block) => {
            collect_called_in_block(block, reachable, worklist);
        }
        ExecExpression::Operation1(_, inner) => {
            collect_called_in_expr(&inner.expression, reachable, worklist);
        }
        ExecExpression::Operation2(_, left, right) => {
            collect_called_in_expr(&left.expression, reachable, worklist);
            collect_called_in_expr(&right.expression, reachable, worklist);
        }
        ExecExpression::BuiltinFunction(_, args) => {
            for arg in args {
                collect_called_in_expr(&arg.expression, reachable, worklist);
            }
        }
        ExecExpression::ArrayAccess(_, index_expr, _) => {
            collect_called_in_expr(&index_expr.expression, reachable, worklist);
        }
        // Factor, Variable, InternalBuiltinFunction はリーフノード
        ExecExpression::Factor(_)
        | ExecExpression::Variable(_)
        | ExecExpression::InternalBuiltinFunction(_) => {}
    }
}
