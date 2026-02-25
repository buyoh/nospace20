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

        // 配列アクセス
        ExecExpression::ArrayAccess(var_ref, index_expr, _array_size) => {
            generate_array_access(ctx, var_ref, index_expr)
        }

        // 単項演算
        ExecExpression::Operation1(op, inner) => generate_unary_op(ctx, op, inner),

        // 二項演算
        ExecExpression::Operation2(op, left, right) => generate_binary_op(ctx, op, left, right),

        // 組み込み関数呼び出し
        // Phase 6: BuiltinFunctionKind enum を使用
        ExecExpression::BuiltinFunction(kind, args) => generate_function_call(ctx, kind, args),

        // Phase 5: ユーザー定義関数呼び出し
        ExecExpression::UserFunction(func_ref, args) => {
            let mut prog = WsProgram::new();

            // 引数を評価してスタックにプッシュ（順番通り）
            // 関数定義では逆順で取得するため、ここでは普通の順序でプッシュする
            for arg in args {
                prog.append(generate_expression(ctx, arg)?);
            }

            // 関数ラベルを関数インデックスで取得または作成
            // func_ref.local_index はグローバル関数インデックスに対応
            let func_label = ctx.get_or_create_function_label(func_ref.local_index);

            // Call 命令を生成
            prog.push(Instruction::Call(func_label));

            // 戻り値がスタックに残る
            Ok(prog)
        }

        // if 式
        ExecExpression::If(cond, then_block, else_block) => {
            generate_if_expression(ctx, cond, then_block, else_block)
        }

        // while 式
        ExecExpression::While(cond, body) => generate_while_expression(ctx, cond, body),

        // ブロック式
        ExecExpression::Block(block) => super::statement::generate_block(ctx, block),
    }
}

/// 変数のアドレスを取得（スタックにアドレスをプッシュ）
fn generate_variable_address(
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
        }
        VarScope::Local => {
            // ローカル: heap[LocalHeapBegin] + offset
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
        }
    }

    Ok(prog)
}

/// 変数の値をロード（スタックにプッシュ）
fn generate_load_variable(
    ctx: &CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
) -> Result<WsProgram, CompileError> {
    let mut prog = generate_variable_address(ctx, var_ref)?;
    prog.push(Instruction::Retrieve);
    Ok(prog)
}

/// 配列要素のアドレスを取得（スタックにアドレスをプッシュ）
fn generate_array_element_address(
    ctx: &mut CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
    index_expr: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let var_info = ctx.get_var_info(var_ref);
    let mut prog = WsProgram::new();

    match var_info.scope {
        VarScope::Global => {
            // global_addr = GLOBAL_PTR + offset + index
            let base_addr = heap_layout::GLOBAL_PTR + var_info.offset;
            prog.push(Instruction::Push(WsNumber(base_addr)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
        }
        VarScope::Local => {
            // local_addr = heap[LOCAL_HEAP_BEGIN] + offset + index
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
        }
    }

    Ok(prog)
}

/// 配列アクセスのコード生成（読み取り）
/// arr[index] の値をスタックにプッシュ
fn generate_array_access(
    ctx: &mut CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
    index_expr: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = generate_array_element_address(ctx, var_ref, index_expr)?;
    prog.push(Instruction::Retrieve);
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
            // 変数または配列要素のアドレスを取得
            match inner {
                ExecExpression::Variable(var_ref) => {
                    prog.append(generate_variable_address(ctx, var_ref)?);
                }
                ExecExpression::ArrayAccess(var_ref, index_expr, _) => {
                    prog.append(generate_array_element_address(ctx, var_ref, index_expr)?);
                }
                _ => {
                    return Err(CompileError::InvalidOperation(
                        "Reference operator (&) can only be applied to variables or array elements"
                            .to_string(),
                    ));
                }
            }
        }
        Operator1::Deref => {
            // スタックトップの値をアドレスとして値を取得
            prog.append(generate_expression(ctx, inner)?);
            prog.push(Instruction::Retrieve);
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
            // 左辺は変数参照、配列アクセス、またはデリファレンスである必要がある
            match left {
                ExecExpression::Variable(var_ref) => {
                    prog.append(generate_store_variable(ctx, var_ref, right)?);
                }
                ExecExpression::ArrayAccess(var_ref, index_expr, _) => {
                    prog.append(generate_store_array(ctx, var_ref, index_expr, right)?);
                }
                ExecExpression::Operation1(Operator1::Deref, addr_expr) => {
                    // デリファレンス代入: *ptr = value
                    // アドレスを評価
                    prog.append(generate_expression(ctx, addr_expr)?);
                    // 値を評価
                    prog.append(generate_expression(ctx, right)?);
                    // Store: heap[addr] = value
                    prog.push(Instruction::Store);
                    // 代入式の値として value を残す（再度取得）
                    prog.append(generate_expression(ctx, addr_expr)?);
                    prog.push(Instruction::Retrieve);
                }
                _ => {
                    return Err(CompileError::InvalidOperation(
                        "Left-hand side of assignment must be a variable, array access, or dereference"
                            .to_string(),
                    ));
                }
            }
        }

        // 複合代入演算子はセマンティック解析で展開されるため、ここに到達することはない
        Operator2::PlusAssign
        | Operator2::MinusAssign
        | Operator2::MultiplyAssign
        | Operator2::DivideAssign
        | Operator2::ModuloAssign => {
            unreachable!("compound assignment operators should be expanded in semantic analysis")
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

/// 配列要素への値の格納
/// arr[index] = value
fn generate_store_array(
    ctx: &mut CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
    index_expr: &ExecExpression,
    value_expr: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let var_info = ctx.get_var_info(var_ref);
    let mut prog = WsProgram::new();

    match var_info.scope {
        VarScope::Global => {
            // global_addr = GLOBAL_PTR + offset + index
            let base_addr = heap_layout::GLOBAL_PTR + var_info.offset;
            prog.push(Instruction::Push(WsNumber(base_addr)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
            // 値を評価してストア
            prog.append(generate_expression(ctx, value_expr)?);
            prog.push(Instruction::Store);
            // 代入式の値として value を残す（再度取得）
            prog.push(Instruction::Push(WsNumber(base_addr)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
            prog.push(Instruction::Retrieve);
        }
        VarScope::Local => {
            // local_addr = heap[LOCAL_HEAP_BEGIN] + offset + index
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
            // 値を評価してストア
            prog.append(generate_expression(ctx, value_expr)?);
            prog.push(Instruction::Store);
            // 代入式の値として value を残す（再度取得）
            prog.push(Instruction::Push(WsNumber(var_info.offset)));
            prog.append(generate_expression(ctx, index_expr)?);
            prog.push(Instruction::Add);
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
    kind: &crate::semantic_analyzer::BuiltinFunctionKind,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    use crate::semantic_analyzer::BuiltinFunctionKind;

    match kind {
        BuiltinFunctionKind::Puti => generate_builtin_puti(ctx, args),
        BuiltinFunctionKind::Putc => generate_builtin_putc(ctx, args),
        BuiltinFunctionKind::Geti => generate_builtin_geti(ctx, args),
        BuiltinFunctionKind::Getc => generate_builtin_getc(ctx, args),
        // デバッグ用組み込み関数: --std-ext debug 時は拡張 API を使用
        BuiltinFunctionKind::Clog => generate_builtin_debug_noop(ctx, args),
        BuiltinFunctionKind::Trace => {
            if ctx.is_debug_ext() {
                generate_builtin_debug_store(ctx, args, heap_layout::EXT_TRACE_ADDR)
            } else {
                generate_builtin_debug_noop(ctx, args)
            }
        }
        BuiltinFunctionKind::Assert => {
            if ctx.is_debug_ext() {
                generate_builtin_debug_store(ctx, args, heap_layout::EXT_ASSERT_ADDR)
            } else {
                generate_builtin_debug_noop(ctx, args)
            }
        }
        BuiltinFunctionKind::AssertNot => {
            if ctx.is_debug_ext() {
                generate_builtin_debug_store(ctx, args, heap_layout::EXT_ASSERT_NOT_ADDR)
            } else {
                generate_builtin_debug_noop(ctx, args)
            }
        }
        BuiltinFunctionKind::Alloc => generate_builtin_alloc(ctx, args),
        BuiltinFunctionKind::Free => generate_builtin_free(ctx, args),
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

/// デバッグ用組み込み関数（--std-ext debug 有効時）
/// __trace(n), __assert(n), __assert_not(n)
/// 引数を評価し、その値を返しつつ、指定された負ヒープアドレスに Store する
fn generate_builtin_debug_store(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
    addr: i64,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    if args.is_empty() {
        // 引数なし: 0 を Store して 0 を返す
        prog.push(Instruction::Push(WsNumber(addr)));
        prog.push(Instruction::Push(WsNumber(0)));
        prog.push(Instruction::Store);
        prog.push(Instruction::Push(WsNumber(0)));
    } else {
        // 最初の引数を評価 → スタック: [..., val]
        prog.append(generate_expression(ctx, &args[0])?);

        // 値を複製（戻り値用） → スタック: [..., val, val]
        prog.push(Instruction::Duplicate);

        // アドレスをプッシュ → スタック: [..., val, val, addr]
        prog.push(Instruction::Push(WsNumber(addr)));

        // swap → スタック: [..., val, addr, val]
        prog.push(Instruction::Swap);

        // store: heap[addr] = val → スタック: [..., val]
        prog.push(Instruction::Store);

        // 残りの引数を評価して破棄（副作用のため）
        for arg in &args[1..] {
            prog.append(generate_expression(ctx, arg)?);
            prog.push(Instruction::Discard);
        }
    }

    Ok(prog)
}

/// __alloc(size) - メモリ確保 (--std-ext alloc 必須)
///
/// スタック出力: [ptr] (確保されたメモリの先頭アドレス)
fn generate_builtin_alloc(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !ctx.is_alloc_ext() {
        return Err(CompileError::InvalidOperation(
            "__alloc requires --std-ext alloc".to_string(),
        ));
    }
    if args.len() != 1 {
        return Err(CompileError::InvalidOperation(format!(
            "__alloc expects 1 argument, got {}",
            args.len()
        )));
    }

    let mut prog = WsProgram::new();
    // 引数 (size) を評価
    prog.append(generate_expression(ctx, &args[0])?);
    // __rt_alloc(size) → ptr
    prog.push(Instruction::Call(reserved_labels::RT_ALLOC));
    Ok(prog)
}

/// __free(ptr) - メモリ解放 (--std-ext alloc 必須)
///
/// スタック出力: [0] (式としての戻り値)
fn generate_builtin_free(
    ctx: &mut CodeGenContext,
    args: &[Box<ExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !ctx.is_alloc_ext() {
        return Err(CompileError::InvalidOperation(
            "__free requires --std-ext alloc".to_string(),
        ));
    }
    if args.len() != 1 {
        return Err(CompileError::InvalidOperation(format!(
            "__free expects 1 argument, got {}",
            args.len()
        )));
    }

    let mut prog = WsProgram::new();
    // 引数 (ptr) を評価
    prog.append(generate_expression(ctx, &args[0])?);
    // __rt_free(ptr)
    prog.push(Instruction::Call(reserved_labels::RT_FREE));
    // 戻り値として 0 をスタックに積む（式としての値）
    prog.push(Instruction::Push(WsNumber(0)));
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

    // ループラベルをスタックにプッシュ (break/continue のため)
    ctx.push_loop_labels(loop_start, loop_end);

    // ループ開始ラベル
    prog.push(Instruction::Label(loop_start));

    // 条件評価
    prog.append(generate_expression(ctx, cond)?);

    // ゼロ（偽）ならループ終了へジャンプ
    prog.push(Instruction::JumpIfZero(loop_end));

    // ループ本体
    prog.append(super::statement::generate_block(ctx, body)?);

    // ブロック値をクリーンアップ（Bug C 修正: while ループ本体のスタックリーク防止）
    prog.push(Instruction::Discard);

    // ループ開始へジャンプ
    prog.push(Instruction::Jump(loop_start));

    // ループ終了ラベル
    prog.push(Instruction::Label(loop_end));

    // ループラベルをポップ
    ctx.pop_loop_labels();

    // while式の値として0を返す
    prog.push(Instruction::Push(WsNumber(0)));

    Ok(prog)
}
