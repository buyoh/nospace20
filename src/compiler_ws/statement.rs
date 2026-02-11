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

    // 注: 現在は main 関数のみを明示的に生成
    // Phase 6: main_function_index を使用してインデックスベースでアクセス
    if let Some(main_idx) = scope.main_function_index {
        let main_func = &scope.functions[main_idx];
        prog.append(generate_function_definition(ctx, "main", main_func)?);
    }

    Ok(prog)
}

/// ブロックのコードを生成
pub fn generate_block(ctx: &mut CodeGenContext, block: &Block) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // ブロック内の文を順次実行
    for stmt in &block.statements {
        prog.append(generate_statement(ctx, stmt)?);
    }

    // ブロックの値として 0 を返す（必要に応じて）
    prog.push(Instruction::Push(WsNumber(0)));

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

        // break/continue (現在未実装)
        ExecStatement::Break => Err(CompileError::InvalidOperation(
            "break not implemented".to_string(),
        )),
        ExecStatement::Continue => Err(CompileError::InvalidOperation(
            "continue not implemented".to_string(),
        )),
    }
}

/// return 文のコード生成
fn generate_return(
    ctx: &CodeGenContext,
    expr: &crate::semantic_analyzer::ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    // 返り値を評価
    prog.append(expression::generate_expression(&mut ctx.clone(), expr)?);

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
    let local_var_count = func.block.scope.variable_count;
    let mut local_ctx = ctx.enter_function(local_var_count);
    prog.append(builtin::generate_local_allocate(
        local_ctx.local_heap_size(),
    ));

    // 引数をローカル変数にコピー
    // 引数はスタックから取得（逆順）
    for i in (0..func.arg_indices.len()).rev() {
        // スタックから引数を取得してローカル変数に格納
        let offset = func.arg_indices.get(i).copied().unwrap_or(i) as i64;
        prog.extend([
            Instruction::Push(WsNumber(offset)),
            Instruction::Push(WsNumber(
                crate::compiler_ws::memory::heap_layout::LOCAL_HEAP_BEGIN,
            )),
            Instruction::Retrieve,
            Instruction::Add,
            Instruction::Swap,
            Instruction::Store,
        ]);
    }

    // 関数本体
    for stmt in &func.block.statements {
        prog.append(generate_statement(&mut local_ctx, stmt)?);
    }

    // デフォルト return（値 0）
    prog.append(builtin::generate_local_deallocate());
    prog.push(Instruction::Push(WsNumber(0)));
    prog.push(Instruction::Return);

    // 関数定義終了ラベル
    prog.push(Instruction::Label(label.offset(1)));

    Ok(prog)
}
