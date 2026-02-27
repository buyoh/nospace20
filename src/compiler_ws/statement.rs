//! 文のコード生成

use crate::compiler_ws::{
    context::CodeGenContext, expression, instruction::Instruction, program::WsProgram,
    types::WsNumber, CompileError, CompileErrorKind,
};
use crate::semantic_analyzer::{Block, ConditionMode, ExecStatement, LocatedExecStatement, Scope};

/// スコープ全体のコードを生成
pub fn generate_scope(ctx: &mut CodeGenContext, scope: &Scope) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // ① ルートレベルの static 変数の初期化を先に実行
    for located_stmt in &scope.static_init_statements {
        ctx.set_location(&located_stmt.location);
        prog.append(generate_statement(ctx, &located_stmt.statement)?);
    }

    // ② 関数内 static 変数の初期化
    for (func_idx, func) in scope.functions.iter().enumerate() {
        // ダミー関数（未到達関数）はスキップ
        if func.is_dummy() {
            continue;
        }
        if !func.block.scope.static_init_statements.is_empty() {
            let func_scope_var_count = func.block.scope.variable_count;
            let total_var_count = calculate_total_variable_count(&func.block);
            let mut static_ctx = ctx.enter_function_for_static_init(
                total_var_count,
                func_scope_var_count,
                func_idx,
                &func.block.scope,
            );
            for located_stmt in &func.block.scope.static_init_statements {
                static_ctx.set_location(&located_stmt.location);
                prog.append(generate_statement(&mut static_ctx, &located_stmt.statement)?);
            }
            ctx.sync_labels_from(&static_ctx);
        }
    }

    // ③ グローバル変数の初期化（root_statements）
    for located_stmt in &scope.root_statements {
        ctx.set_location(&located_stmt.location);
        prog.append(generate_statement(ctx, &located_stmt.statement)?);
    }

    // ④ Phase 7: 全ての関数定義を生成
    // 関数は symbol_table.function_names と functions が対応している
    for (i, func_name) in scope.symbol_table.function_names.iter().enumerate() {
        let func = &scope.functions[i];
        // ダミー関数（未到達関数）はコード生成をスキップ
        if func.is_dummy() {
            continue;
        }
        prog.append(generate_function_definition(ctx, func_name, func, i)?);
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
        let located = &block.statements[i];
        ctx.set_location(&located.location);
        prog.append(generate_statement(ctx, &located.statement)?);
    }

    // 最後の文を処理
    let last_located = &block.statements[stmt_count - 1];
    ctx.set_location(&last_located.location);
    let last_stmt = &last_located.statement;
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
        ExecStatement::Return(Some(expr)) => generate_return(ctx, expr),
        ExecStatement::Return(None) => generate_void_return(ctx),

        // break 文
        ExecStatement::Break => {
            let loop_end = ctx
                .current_loop_end()
                .ok_or_else(|| {
                    let loc = ctx.current_location();
                    match loc {
                        Some(l) => CompileError::with_location(
                            CompileErrorKind::InvalidOperation("break outside loop".to_string()),
                            l,
                        ),
                        None => CompileError::new(CompileErrorKind::InvalidOperation(
                            "break outside loop".to_string(),
                        )),
                    }
                })?;
            let mut prog = WsProgram::new();
            prog.push(Instruction::Jump(loop_end));
            Ok(prog)
        }

        // continue 文
        ExecStatement::Continue => {
            let loop_start = ctx.current_loop_start().ok_or_else(|| {
                let loc = ctx.current_location();
                match loc {
                    Some(l) => CompileError::with_location(
                        CompileErrorKind::InvalidOperation("continue outside loop".to_string()),
                        l,
                    ),
                    None => CompileError::new(CompileErrorKind::InvalidOperation(
                        "continue outside loop".to_string(),
                    )),
                }
            })?;
            let mut prog = WsProgram::new();
            prog.push(Instruction::Jump(loop_start));
            Ok(prog)
        }

        // while 文
        ExecStatement::While(mode, cond, body) => {
            generate_while_statement(ctx, mode, cond, body)
        }
    }
}

/// while 文のコード生成
///
/// 式版との差異: ループ終了後に Push(0) しない（値を返す必要がない）
fn generate_while_statement(
    ctx: &mut CodeGenContext,
    mode: &ConditionMode,
    cond: &crate::semantic_analyzer::LocatedExecExpression,
    body: &Block,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    let loop_start = ctx.new_label();
    let loop_end = ctx.new_label();

    // ループラベルをスタックにプッシュ (break/continue のため)
    ctx.push_loop_labels(loop_start, loop_end);

    // ループ開始ラベル
    prog.push(Instruction::Label(loop_start));

    // 条件評価
    prog.append(expression::generate_expression(ctx, cond)?);

    // ConditionMode に応じたループ終了ジャンプ命令
    match mode {
        ConditionMode::NonZero => {
            // cond == 0 (偽) ならループ終了
            prog.push(Instruction::JumpIfZero(loop_end));
        }
        ConditionMode::Zero => {
            // cond == 0 → ループ継続 なので、cond != 0 ならループ終了
            let continue_label = ctx.new_label();
            prog.push(Instruction::JumpIfZero(continue_label));
            prog.push(Instruction::Jump(loop_end));
            prog.push(Instruction::Label(continue_label));
        }
        ConditionMode::Negative => {
            // cond < 0 → ループ継続 なので、cond >= 0 ならループ終了
            let continue_label = ctx.new_label();
            prog.push(Instruction::JumpIfNegative(continue_label));
            prog.push(Instruction::Jump(loop_end));
            prog.push(Instruction::Label(continue_label));
        }
    }

    // ループ本体
    prog.append(generate_block(ctx, body)?);

    // ブロック値をクリアアップ（generate_block は常に値をプッシュする）
    prog.push(Instruction::Discard);

    // ループ開始へジャンプ
    prog.push(Instruction::Jump(loop_start));

    // ループ終了ラベル
    prog.push(Instruction::Label(loop_end));

    ctx.pop_loop_labels();

    // 注: 式版では Push(0) していたが、文版では不要
    Ok(prog)
}

/// return 文のコード生成
fn generate_return(
    ctx: &mut CodeGenContext,
    expr: &crate::semantic_analyzer::LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // 返り値を評価
    prog.append(expression::generate_expression(ctx, expr)?);

    // Fix B: deallocateの前にswapを挿入
    // stack: [old_LHB, return_value] -> [return_value, old_LHB]
    prog.push(Instruction::Swap);

    // ローカル変数領域解放（AllocRuntime 経由）
    prog.append(ctx.alloc_runtime().generate_function_epilogue());

    // 関数から戻る
    prog.push(Instruction::Return);

    Ok(prog)
}

/// void return 文のコード生成（式なし）
fn generate_void_return(ctx: &mut CodeGenContext) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // デフォルト返却値 0
    prog.push(Instruction::Push(WsNumber(0)));

    // Fix B: deallocateの前にswapを挿入
    // stack: [old_LHB, return_value] -> [return_value, old_LHB]
    prog.push(Instruction::Swap);

    // ローカル変数領域解放（AllocRuntime 経由）
    prog.append(ctx.alloc_runtime().generate_function_epilogue());

    // 関数から戻る
    prog.push(Instruction::Return);

    Ok(prog)
}

/// 関数定義のコード生成
fn generate_function_definition(
    ctx: &mut CodeGenContext,
    _func_name: &str,
    func: &crate::semantic_analyzer::Function,
    func_index: usize,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    let label = ctx.get_or_create_function_label(func_index);

    // 関数本体をスキップするジャンプ
    prog.push(Instruction::Jump(label.offset(1)));

    // 関数エントリポイント
    prog.push(Instruction::Label(label));

    // ローカル変数領域確保
    let func_scope_var_count = func.block.scope.variable_count;
    let total_var_count = calculate_total_variable_count(&func.block);
    let mut local_ctx = ctx.enter_function(
        total_var_count,
        func_scope_var_count,
        func_index,
        &func.block.scope,
    );

    // 引数オフセットを計算
    let arg_offsets: Vec<i64> = (0..func.arg_indices.len())
        .map(|i| func.arg_indices.get(i).copied().unwrap_or(i) as i64)
        .collect();

    // 関数プロローグ: 引数コピー + フレーム確保（AllocRuntime 経由）
    prog.append(
        local_ctx
            .alloc_runtime()
            .generate_function_prologue(local_ctx.local_heap_size(), &arg_offsets),
    );

    // 関数本体
    for located_stmt in &func.block.statements {
        local_ctx.set_location(&located_stmt.location);
        prog.append(generate_statement(&mut local_ctx, &located_stmt.statement)?);
    }

    // デフォルト return（値 0）
    prog.append(local_ctx.alloc_runtime().generate_function_epilogue());
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

fn count_nested_vars_in_statements(stmts: &[LocatedExecStatement]) -> usize {
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
        ExecStatement::Return(None) | ExecStatement::Break | ExecStatement::Continue => 0,
    }
}

fn count_nested_vars_in_expression(located_expr: &crate::semantic_analyzer::LocatedExecExpression) -> usize {
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
        ExecExpression::Variable(_) | ExecExpression::Factor(_) => 0,
        ExecExpression::InternalBuiltinFunction(_) => 0,
    }
}
