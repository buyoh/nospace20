//! 式のコード生成

use crate::compiler_ws::{
    context::CodeGenContext,
    instruction::Instruction,
    label::reserved_labels,
    memory::heap_layout,
    program::WsProgram,
    types::WsNumber,
    CompileError,
    context::VarScope,
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
        ExecExpression::Variable(var_ref) => {
            generate_load_variable(ctx, var_ref)
        }
        
        // 単項演算
        ExecExpression::Operation1(op, inner) => {
            generate_unary_op(ctx, op, inner)
        }
        
        // 二項演算
        ExecExpression::Operation2(op, left, right) => {
            generate_binary_op(ctx, op, left, right)
        }
        
        // 関数呼び出し
        ExecExpression::Function(func_name, args) => {
            generate_function_call(ctx, func_name, args)
        }
        
        // if 式
        ExecExpression::If(cond, then_block, else_block) => {
            generate_if_expression(ctx, cond, then_block, else_block)
        }
        
        // while 式
        ExecExpression::While(cond, body) => {
            generate_while_expression(ctx, cond, body)
        }
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
                    "Left-hand side of assignment must be a variable".to_string()
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
    _ctx: &mut CodeGenContext,
    func_name: &str,
    _args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    // TODO: 組み込み関数と ユーザー定義関数の実装
    Err(CompileError::UndefinedFunction(func_name.to_string()))
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
