//! 式のコード生成

use crate::compiler_ws::{
    context::CodeGenContext, context::VarScope, instruction::Instruction, label::reserved_labels,
    memory::heap_layout, program::WsProgram, types::WsNumber, CompileError,
};
use crate::semantic_analyzer::ExecExpression;
use crate::tree_parser::{Operator1, Operator2};

/// 式を評価するコードを生成
/// 評価結果はスタックトップに残る
pub fn generate_expression(
    ctx: &mut CodeGenContext,
    expr: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    match expr {
        // リテラル値
        ExecExpression::Factor(value) => {
            let mut prog = WsProgram::new();
            prog.push(Instruction::Push(WsNumber(*value)));
            Ok(prog)
        }

        // 変数参照
        ExecExpression::Variable(var_ref) => generate_load_variable(ctx, var_ref),

        // 単項演算
        ExecExpression::Operation1(op, inner) => generate_unary_op(ctx, op, inner),

        // 二項演算
        ExecExpression::Operation2(op, left, right) => generate_binary_op(ctx, op, left, right),

        // 関数呼び出し
        ExecExpression::Function(func_name, args) => generate_function_call(ctx, func_name, args),

        // if 式
        ExecExpression::If(cond, then_block, else_block) => {
            generate_if_expression(ctx, cond, then_block, else_block)
        }

        // while 式
        ExecExpression::While(cond, body) => generate_while_expression(ctx, cond, body),
    }
}

/// 変数の値をロード（スタックにプッシュ）
fn generate_load_variable(
    ctx: &CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
) -> Result<WsProgram, CompileError> {
    let var_info = ctx.get_var_info(var_ref);
    let mut prog = WsProgram::new();

    match var_info.scope {
        VarScope::Global => {
            // グローバル: GlobalPtr + offset
            let addr = heap_layout::GLOBAL_PTR + var_info.offset;
            prog.push(Instruction::Push(WsNumber(addr)));
            prog.push(Instruction::Retrieve);
        }
        VarScope::Local => {
            // ローカル: heap[LocalHeapBegin] + offset
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
            prog.push(Instruction::Retrieve);
        }
    }

    Ok(prog)
}

/// 単項演算子のコード生成
fn generate_unary_op(
    ctx: &mut CodeGenContext,
    op: &Operator1,
    inner: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    match op {
        Operator1::Negative => {
            // 0 - value
            prog.push(Instruction::Push(WsNumber(0)));
            prog.append(generate_expression(ctx, inner)?);
            prog.push(Instruction::Sub);
        }
        Operator1::LogicalNot => {
            // value == 0 ? 1 : 0
            prog.push(Instruction::Push(WsNumber(1))); // zero → true
            prog.push(Instruction::Push(WsNumber(0))); // non-zero → false
            prog.append(generate_expression(ctx, inner)?);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_ZERO));
        }
        Operator1::Ref => {
            // Phase 4 で実装予定
            unimplemented!("reference operator (&) is not implemented yet")
        }
        Operator1::Deref => {
            // Phase 4 で実装予定
            unimplemented!("dereference operator (*) is not implemented yet")
        }
    }

    Ok(prog)
}

/// 二項演算子のコード生成
fn generate_binary_op(
    ctx: &mut CodeGenContext,
    op: &Operator2,
    left: &ExecExpression,
    right: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    match op {
        // 算術演算
        Operator2::Plus => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Add);
        }
        Operator2::Minus => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
        }
        Operator2::Multiply => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Mul);
        }
        Operator2::Divide => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Div);
        }
        Operator2::Modulo => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Mod);
        }

        // 比較演算
        Operator2::Equal => {
            prog.push(Instruction::Push(WsNumber(1))); // zero → true
            prog.push(Instruction::Push(WsNumber(0))); // non-zero → false
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_ZERO));
        }
        Operator2::NotEqual => {
            prog.push(Instruction::Push(WsNumber(0))); // zero → false
            prog.push(Instruction::Push(WsNumber(1))); // non-zero → true
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_ZERO));
        }
        Operator2::Less => {
            prog.push(Instruction::Push(WsNumber(1))); // negative → true
            prog.push(Instruction::Push(WsNumber(0))); // non-negative → false
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_NEGATIVE));
        }
        Operator2::LessEqual => {
            // left <= right ⇔ !(left > right) ⇔ !(right < left)
            prog.push(Instruction::Push(WsNumber(0))); // negative → false
            prog.push(Instruction::Push(WsNumber(1))); // non-negative → true
            prog.append(generate_expression(ctx, right)?);
            prog.append(generate_expression(ctx, left)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_NEGATIVE));
        }
        Operator2::Greater => {
            // left > right ⇔ right < left
            prog.push(Instruction::Push(WsNumber(1))); // negative → true
            prog.push(Instruction::Push(WsNumber(0))); // non-negative → false
            prog.append(generate_expression(ctx, right)?);
            prog.append(generate_expression(ctx, left)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_NEGATIVE));
        }
        Operator2::GreaterEqual => {
            // left >= right ⇔ !(left < right)
            prog.push(Instruction::Push(WsNumber(0))); // negative → false
            prog.push(Instruction::Push(WsNumber(1))); // non-negative → true
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_NEGATIVE));
        }

        // 論理演算
        Operator2::LogicalAnd => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_AND));
        }
        Operator2::LogicalOr => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_OR));
        }

        // 代入
        Operator2::Assign => {
            // 左辺は変数参照である必要がある
            if let ExecExpression::Variable(var_ref) = left {
                prog.append(generate_store_variable(ctx, var_ref, right)?);
            } else {
                return Err(CompileError::InvalidOperation(
                    "Left-hand side of assignment must be a variable".to_string(),
                ));
            }
        }
    }

    Ok(prog)
}

/// 変数への値の格納
fn generate_store_variable(
    ctx: &mut CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
    value_expr: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let var_info = ctx.get_var_info(var_ref);
    let mut prog = WsProgram::new();

    match var_info.scope {
        VarScope::Global => {
            // グローバル: heap[GlobalPtr + offset] = value
            let addr = heap_layout::GLOBAL_PTR + var_info.offset;
            prog.push(Instruction::Push(WsNumber(addr)));
            prog.append(generate_expression(ctx, value_expr)?);
            prog.push(Instruction::Store);
            // 代入式の値として value を残す
            prog.push(Instruction::Push(WsNumber(addr)));
            prog.push(Instruction::Retrieve);
        }
        VarScope::Local => {
            // ローカル: heap[heap[LocalHeapBegin] + offset] = value
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
            prog.append(generate_expression(ctx, value_expr)?);
            prog.push(Instruction::Store);
            // 代入式の値として value を残す（再度取得）
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
            prog.push(Instruction::Retrieve);
        }
    }

    Ok(prog)
}

/// 関数呼び出し
fn generate_function_call(
    ctx: &mut CodeGenContext,
    func_name: &str,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    match func_name {
        "__puti" => generate_builtin_puti(ctx, args),
        "__putc" => generate_builtin_putc(ctx, args),
        "__geti" => generate_builtin_geti(ctx, args),
        "__getc" => generate_builtin_getc(ctx, args),
        // デバッグ用組み込み関数は Whitespace では無視（引数は評価して値を返す）
        "__clog" => generate_builtin_debug_noop(ctx, args),
        "__trace" => generate_builtin_debug_noop(ctx, args),
        "__assert" => generate_builtin_debug_noop(ctx, args),
        "__assert_not" => generate_builtin_debug_noop(ctx, args),
        _ => {
            // TODO: ユーザー定義関数の実装
            Err(CompileError::UndefinedFunction(func_name.to_string()))
        }
    }
}

/// __puti(x) - 整数を10進数で出力
fn generate_builtin_puti(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    if args.len() != 1 {
        return Err(CompileError::InvalidOperation(format!(
            "__puti expects 1 argument, got {}",
            args.len()
        )));
    }

    let mut prog = WsProgram::new();
    // 引数を評価
    prog.append(generate_expression(ctx, &args[0])?);
    // 値を複製（戻り値用）
    prog.push(Instruction::Duplicate);
    // 整数として出力
    prog.push(Instruction::OutputNumber);
    Ok(prog)
}

/// __putc(x) - 文字を出力
fn generate_builtin_putc(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    if args.len() != 1 {
        return Err(CompileError::InvalidOperation(format!(
            "__putc expects 1 argument, got {}",
            args.len()
        )));
    }

    let mut prog = WsProgram::new();
    // 引数を評価
    prog.append(generate_expression(ctx, &args[0])?);
    // 値を複製（戻り値用）
    prog.push(Instruction::Duplicate);
    // 文字として出力
    prog.push(Instruction::OutputChar);
    Ok(prog)
}

/// __geti() - 整数を入力
fn generate_builtin_geti(
    _ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !args.is_empty() {
        return Err(CompileError::InvalidOperation(format!(
            "__geti expects 0 arguments, got {}",
            args.len()
        )));
    }

    let mut prog = WsProgram::new();
    // 一時領域のアドレスをプッシュ
    prog.push(Instruction::Push(WsNumber(heap_layout::TEMP_PTR)));
    // アドレスを複製（InputNumber用とRetrieve用）
    prog.push(Instruction::Duplicate);
    // 整数を入力してheap[TEMP_PTR]に格納
    prog.push(Instruction::InputNumber);
    // heap[TEMP_PTR]の値をスタックに取り出す
    prog.push(Instruction::Retrieve);
    Ok(prog)
}

/// __getc() - 文字を入力
fn generate_builtin_getc(
    _ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !args.is_empty() {
        return Err(CompileError::InvalidOperation(format!(
            "__getc expects 0 arguments, got {}",
            args.len()
        )));
    }

    let mut prog = WsProgram::new();
    // 一時領域のアドレスをプッシュ
    prog.push(Instruction::Push(WsNumber(heap_layout::TEMP_PTR)));
    // アドレスを複製（InputChar用とRetrieve用）
    prog.push(Instruction::Duplicate);
    // 文字を入力してheap[TEMP_PTR]に格納
    prog.push(Instruction::InputChar);
    // heap[TEMP_PTR]の値をスタックに取り出す
    prog.push(Instruction::Retrieve);
    Ok(prog)
}

/// デバッグ用組み込み関数（Whitespace では無視）
/// __clog, __trace, __assert, __assert_not
/// 引数を評価して、最初の引数の値を返す（引数がない場合は 0 を返す）
fn generate_builtin_debug_noop(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    if args.is_empty() {
        // 引数なしの場合は 0 を返す
        prog.push(Instruction::Push(WsNumber(0)));
    } else {
        // 最初の引数を評価（戻り値として使用）
        prog.append(generate_expression(ctx, &args[0])?);

        // 残りの引数を評価（副作用のため）して破棄
        for arg in &args[1..] {
            prog.append(generate_expression(ctx, arg)?);
            prog.push(Instruction::Discard);
        }
    }

    Ok(prog)
}

/// if 式
fn generate_if_expression(
    ctx: &mut CodeGenContext,
    cond: &ExecExpression,
    then_block: &crate::semantic_analyzer::Block,
    else_block: &crate::semantic_analyzer::Block,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    let else_label = ctx.new_label();
    let end_label = ctx.new_label();

    // 条件評価
    prog.append(generate_expression(ctx, cond)?);

    // ゼロ（偽）なら else へジャンプ
    prog.push(Instruction::JumpIfZero(else_label));

    // then ブロック
    prog.append(super::statement::generate_block(ctx, then_block)?);
    prog.push(Instruction::Jump(end_label));

    // else ブロック
    prog.push(Instruction::Label(else_label));
    prog.append(super::statement::generate_block(ctx, else_block)?);

    // 終了ラベル
    prog.push(Instruction::Label(end_label));

    Ok(prog)
}

/// while 式
fn generate_while_expression(
    ctx: &mut CodeGenContext,
    cond: &ExecExpression,
    body: &crate::semantic_analyzer::Block,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    let loop_start = ctx.new_label();
    let loop_end = ctx.new_label();

    // ループ開始ラベル
    prog.push(Instruction::Label(loop_start));

    // 条件評価
    prog.append(generate_expression(ctx, cond)?);

    // ゼロ（偽）ならループ終了へジャンプ
    prog.push(Instruction::JumpIfZero(loop_end));

    // ループ本体
    prog.append(super::statement::generate_block(ctx, body)?);

    // ループ開始へジャンプ
    prog.push(Instruction::Jump(loop_start));

    // ループ終了ラベル
    prog.push(Instruction::Label(loop_end));

    // while式の値として0を返す
    prog.push(Instruction::Push(WsNumber(0)));

    Ok(prog)
}
