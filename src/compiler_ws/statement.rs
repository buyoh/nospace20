//! 文のコード生成

use crate::compiler_ws::{
    builtin, context::CodeGenContext, expression, instruction::Instruction, program::WsProgram,
    types::WsNumber, CompileError,
};
use crate::semantic_analyzer::{Block, ExecStatement, Scope};

/// スコープ全体のコードを生成
pub fn generate_scope(ctx: &mut CodeGenContext, scope: &Scope) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // static 変数の初期化を先に実行
    for stmt in &scope.static_init_statements {
        prog.append(generate_statement(ctx, stmt)?);
    }

    // グローバル変数の初期化（root_statements）
    for stmt in &scope.root_statements {
        prog.append(generate_statement(ctx, stmt)?);
    }

    // Phase 7: 全ての関数定義を生成
    // 関数は symbol_table.function_names と functions が対応している
    for (i, func_name) in scope.symbol_table.function_names.iter().enumerate() {
        let func = &scope.functions[i];
        prog.append(generate_function_definition(ctx, func_name, func)?);
    }

    Ok(prog)
}

/// ブロックのコードを生成
/// ブロック式の値は最後に評価された式の値となる (spec §6.5)
pub fn generate_block(ctx: &mut CodeGenContext, block: &Block) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    ctx.enter_block_scope(block.scope.variable_count);

    let stmt_count = block.statements.len();

    if stmt_count == 0 {
        // 空ブロックは 0 を返す
        ctx.leave_block_scope();
        prog.push(Instruction::Push(WsNumber(0)));
        return Ok(prog);
    }

    // 最後の文以外を処理（式の値は Discard）
    for i in 0..stmt_count - 1 {
        prog.append(generate_statement(ctx, &block.statements[i])?);
    }

    // 最後の文を処理
    let last_stmt = &block.statements[stmt_count - 1];
    match last_stmt {
        ExecStatement::Expression(expr) => {
            // 式文の場合: 値をスタックに残す（Discard しない）
            prog.append(expression::generate_expression(ctx, expr)?);
        }
        _ => {
            // return/break/continue の場合: 通常処理
            // これらはフロー制御を行うため、ブロック式の値は使われないが、
            // スタック整合性のため 0 を push する
            prog.append(generate_statement(ctx, last_stmt)?);
            prog.push(Instruction::Push(WsNumber(0)));
        }
    }

    ctx.leave_block_scope();
    Ok(prog)
}

/// 文を実行するコードを生成
pub fn generate_statement(
    ctx: &mut CodeGenContext,
    stmt: &ExecStatement,
) -> Result<WsProgram, CompileError> {
    match stmt {
        // 式文（結果を破棄）
        ExecStatement::Expression(expr) => {
            let mut prog = expression::generate_expression(ctx, expr)?;
            prog.push(Instruction::Discard);
            Ok(prog)
        }

        // return 文
        ExecStatement::Return(expr) => generate_return(ctx, expr),

        // break 文
        ExecStatement::Break => {
            let loop_end = ctx.current_loop_end().ok_or_else(|| {
                CompileError::InvalidOperation("break outside loop".to_string())
            })?;
            let mut prog = WsProgram::new();
            prog.push(Instruction::Jump(loop_end));
            Ok(prog)
        }

        // continue 文
        ExecStatement::Continue => {
            let loop_start = ctx.current_loop_start().ok_or_else(|| {
                CompileError::InvalidOperation("continue outside loop".to_string())
            })?;
            let mut prog = WsProgram::new();
            prog.push(Instruction::Jump(loop_start));
            Ok(prog)
        }
    }
}

/// return 文のコード生成
fn generate_return(
    ctx: &mut CodeGenContext,
    expr: &crate::semantic_analyzer::ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // 返り値を評価
    prog.append(expression::generate_expression(ctx, expr)?);

    // Fix B: deallocateの前にswapを挿入
    // stack: [old_LHB, return_value] -> [return_value, old_LHB]
    prog.push(Instruction::Swap);

    // ローカル変数領域解放
    prog.append(builtin::generate_local_deallocate());

    // 関数から戻る
    prog.push(Instruction::Return);

    Ok(prog)
}

/// 関数定義のコード生成
fn generate_function_definition(
    ctx: &mut CodeGenContext,
    func_name: &str,
    func: &crate::semantic_analyzer::Function,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    let label = ctx.get_or_create_function_label(func_name);

    // 関数本体をスキップするジャンプ
    prog.push(Instruction::Jump(label.offset(1)));

    // 関数エントリポイント
    prog.push(Instruction::Label(label));

    // ローカル変数領域確保
    let func_scope_var_count = func.block.scope.variable_count;
    let total_var_count = calculate_total_variable_count(&func.block);
    let mut local_ctx = ctx.enter_function(total_var_count, func_scope_var_count);

    // 引数をローカル変数にコピー
    // 引数はスタックから取得（逆順）
    // allocate前のLOCAL_HEAP_ENDは新しいフレームのLOCAL_HEAP_BEGINと同じ値になる
    for i in (0..func.arg_indices.len()).rev() {
        // スタックから引数を取得してローカル変数に格納
        let offset = func.arg_indices.get(i).copied().unwrap_or(i) as i64;
        prog.extend([
            Instruction::Push(WsNumber(offset)),
            Instruction::Push(WsNumber(
                crate::compiler_ws::memory::heap_layout::LOCAL_HEAP_END,
            )),
            Instruction::Retrieve,
            Instruction::Add,
            Instruction::Swap,
            Instruction::Store,
        ]);
    }

    prog.append(builtin::generate_local_allocate(
        local_ctx.local_heap_size(),
    ));

    // 関数本体
    for stmt in &func.block.statements {
        prog.append(generate_statement(&mut local_ctx, stmt)?);
    }

    // デフォルト return（値 0）
    prog.append(builtin::generate_local_deallocate());
    prog.push(Instruction::Push(WsNumber(0)));
    prog.push(Instruction::Return);

    // 子コンテキストのラベルカウンタを親に同期
    ctx.sync_labels_from(&local_ctx);

    // 関数定義終了ラベル
    prog.push(Instruction::Label(label.offset(1)));

    Ok(prog)
}

/// 関数内の全ブロック（ネスト含む）の変数合計数を計算
fn calculate_total_variable_count(block: &Block) -> usize {
    block.scope.variable_count + count_nested_vars_in_statements(&block.statements)
}

fn count_nested_vars_in_statements(stmts: &[ExecStatement]) -> usize {
    stmts.iter().map(count_nested_vars_in_statement).sum()
}

fn count_nested_vars_in_statement(stmt: &ExecStatement) -> usize {
    match stmt {
        ExecStatement::Expression(expr) | ExecStatement::Return(expr) => {
            count_nested_vars_in_expression(expr)
        }
        ExecStatement::Break | ExecStatement::Continue => 0,
    }
}

fn count_nested_vars_in_expression(expr: &crate::semantic_analyzer::ExecExpression) -> usize {
    use crate::semantic_analyzer::ExecExpression;
    match expr {
        ExecExpression::If(cond, then_block, else_block) => {
            count_nested_vars_in_expression(cond)
                + calculate_total_variable_count(then_block)
                + calculate_total_variable_count(else_block)
        }
        ExecExpression::While(cond, body) => {
            count_nested_vars_in_expression(cond) + calculate_total_variable_count(body)
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
        ExecExpression::ArrayAccess(_, index_expr, _) => count_nested_vars_in_expression(index_expr),
        ExecExpression::Variable(_) | ExecExpression::Factor(_) => 0,
    }
}
