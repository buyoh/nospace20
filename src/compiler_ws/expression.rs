//! 式のコード生成

use crate::compiler_ws::{
    context::CodeGenContext, context::VarScope, instruction::Instruction, label::reserved_labels,
    memory::heap_layout, program::WsProgram, types::WsNumber, CompileError, CompileErrorKind,
};
use crate::semantic_analyzer::{
    ConditionMode, ExecExpression, InternalBuiltinFunctionKind, LocatedExecExpression,
};
use crate::tree_parser::{Operator1, Operator2};

/// コンパイルエラーを現在のコンテキスト位置情報付きで生成するヘルパー
///
/// `ctx.current_location()` が Some の場合は位置情報付きのエラーを返す。
/// 式レベルのエラーは直近の文の開始位置で代替表示。
fn make_error(ctx: &CodeGenContext, msg: String) -> CompileError {
    match ctx.current_location() {
        Some(loc) => CompileError::with_location(CompileErrorKind::InvalidOperation(msg), loc),
        None => CompileError::new(CompileErrorKind::InvalidOperation(msg)),
    }
}

/// 式を評価するコードを生成
/// 評価結果はスタックトップに残る
pub fn generate_expression(
    ctx: &mut CodeGenContext,
    located_expr: &LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    ctx.set_location(&located_expr.location);
    let expr = &located_expr.expression;
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
        // BuiltinFunctionKind enum を使用
        ExecExpression::BuiltinFunction(kind, args) => generate_function_call(ctx, kind, args),

        // ユーザー定義関数呼び出し
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
        ExecExpression::If(mode, cond, then_block, else_block) => {
            generate_if_expression(ctx, mode, cond, then_block, else_block)
        }

        // ブロック式
        ExecExpression::Block(block) => super::statement::generate_block(ctx, block),

        // 最適化パスで生成される内部組み込み関数
        ExecExpression::InternalBuiltinFunction(kind) => {
            generate_internal_builtin_function(ctx, kind)
        }
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
    index_expr: &LocatedExecExpression,
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
    index_expr: &LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = generate_array_element_address(ctx, var_ref, index_expr)?;
    prog.push(Instruction::Retrieve);
    Ok(prog)
}

/// 単項演算子のコード生成
fn generate_unary_op(
    ctx: &mut CodeGenContext,
    op: &Operator1,
    inner: &LocatedExecExpression,
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
            match &inner.expression {
                ExecExpression::Variable(var_ref) => {
                    prog.append(generate_variable_address(ctx, var_ref)?);
                }
                ExecExpression::ArrayAccess(var_ref, index_expr, _) => {
                    prog.append(generate_array_element_address(ctx, var_ref, index_expr)?);
                }
                _ => {
                    return Err(make_error(
                        ctx,
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

/// 比較演算で使用するオペランドの選択
enum ComparisonOperand {
    Left,
    Right,
}

/// 比較演算で使用する条件ジャンプの種別
enum ComparisonJumpKind {
    Zero,
    Negative,
}

/// 比較演算子ごとのコード生成仕様
///
/// 演算子の差異（オペランド順序・ジャンプ条件・真偽方向）をデータとして表現することで、
/// 6種の比較演算子を `generate_comparison` 一関数に統一する。
struct ComparisonSpec {
    /// Sub 前に最初に評価するオペランド（Left = left-right、Right = right-left）
    first_operand: ComparisonOperand,
    /// 使用する条件ジャンプ（JumpIfZero または JumpIfNegative）
    jump_kind: ComparisonJumpKind,
    /// 条件ジャンプが成立した場合の値が true かどうか
    jump_is_true: bool,
}

/// 比較演算子から ComparisonSpec を返す
fn comparison_spec(op: &Operator2) -> ComparisonSpec {
    match op {
        // x == y → (x - y) == 0
        Operator2::Equal => ComparisonSpec {
            first_operand: ComparisonOperand::Left,
            jump_kind: ComparisonJumpKind::Zero,
            jump_is_true: true,
        },
        // x != y → (x - y) != 0
        Operator2::NotEqual => ComparisonSpec {
            first_operand: ComparisonOperand::Left,
            jump_kind: ComparisonJumpKind::Zero,
            jump_is_true: false,
        },
        // x < y → (x - y) < 0
        Operator2::Less => ComparisonSpec {
            first_operand: ComparisonOperand::Left,
            jump_kind: ComparisonJumpKind::Negative,
            jump_is_true: true,
        },
        // left <= right ⇔ !(right - left < 0)
        Operator2::LessEqual => ComparisonSpec {
            first_operand: ComparisonOperand::Right,
            jump_kind: ComparisonJumpKind::Negative,
            jump_is_true: false,
        },
        // left > right ⇔ right - left < 0
        Operator2::Greater => ComparisonSpec {
            first_operand: ComparisonOperand::Right,
            jump_kind: ComparisonJumpKind::Negative,
            jump_is_true: true,
        },
        // left >= right ⇔ !(left - right < 0)
        Operator2::GreaterEqual => ComparisonSpec {
            first_operand: ComparisonOperand::Left,
            jump_kind: ComparisonJumpKind::Negative,
            jump_is_true: false,
        },
        _ => unreachable!("comparison_spec called with non-comparison operator"),
    }
}

/// 比較演算のインラインコード生成
///
/// spec に従い、Sub + 条件ジャンプ + Push(true/false) というパターンを生成する。
/// 生成命令数: 約 8 命令（Sub, JumpIfZero/Negative, Push, Jump, Label×2, Push）
fn generate_comparison(
    ctx: &mut CodeGenContext,
    spec: &ComparisonSpec,
    left: &LocatedExecExpression,
    right: &LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    let label_jump = ctx.new_label();
    let label_end = ctx.new_label();

    // オペランドの順序に従って評価
    let (first, second) = match spec.first_operand {
        ComparisonOperand::Left => (left, right),
        ComparisonOperand::Right => (right, left),
    };
    prog.append(generate_expression(ctx, first)?);
    prog.append(generate_expression(ctx, second)?);
    prog.push(Instruction::Sub);

    // 条件ジャンプ
    match spec.jump_kind {
        ComparisonJumpKind::Zero => prog.push(Instruction::JumpIfZero(label_jump)),
        ComparisonJumpKind::Negative => prog.push(Instruction::JumpIfNegative(label_jump)),
    }

    // ジャンプしなかった場合の値（jump_is_true = true なら false = 0）
    prog.push(Instruction::Push(WsNumber(if spec.jump_is_true {
        0
    } else {
        1
    })));
    prog.push(Instruction::Jump(label_end));

    // ジャンプした場合の値（jump_is_true = true なら true = 1）
    prog.push(Instruction::Label(label_jump));
    prog.push(Instruction::Push(WsNumber(if spec.jump_is_true {
        1
    } else {
        0
    })));
    prog.push(Instruction::Label(label_end));

    Ok(prog)
}

/// 二項演算子のコード生成
fn generate_binary_op(
    ctx: &mut CodeGenContext,
    op: &Operator2,
    left: &LocatedExecExpression,
    right: &LocatedExecExpression,
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

        // 比較演算（インライン化・データ駆動）
        //
        // サブルーチン呼び出し（COMPARATOR_ZERO / COMPARATOR_NEGATIVE）の代わりに、
        // JumpIfZero / JumpIfNegative を使ったインライン分岐を生成する（comparison-inline 最適化）。
        // 合計命令数: 約 8 命令（元の 11 命令から削減）
        // 演算子ごとの差異は ComparisonSpec でテーブル化し、generate_comparison に委譲する。
        Operator2::Equal
        | Operator2::NotEqual
        | Operator2::Less
        | Operator2::LessEqual
        | Operator2::Greater
        | Operator2::GreaterEqual => {
            let spec = comparison_spec(op);
            prog.append(generate_comparison(ctx, &spec, left, right)?);
        }

        // 論理演算（短絡評価）
        //
        // バグ修正: 以前は両辺を先に評価してからサブルーチンに渡していたため、
        // 右辺の副作用が仕様に反して常に実行されていた。
        // インライン分岐に変換することで仕様準拠の短絡評価を実現する。
        //
        // `a && b` のインライン展開:
        //   eval(a)
        //   JumpIfZero(false_label)   # a == 0 なら短絡して偽
        //   eval(b)
        //   JumpIfZero(false_label)   # b == 0 なら偽
        //   Push(1)
        //   Jump(end_label)
        //   Label(false_label): Push(0)
        //   Label(end_label)
        Operator2::LogicalAnd => {
            let false_label = ctx.new_label();
            let end_label = ctx.new_label();
            // 左辺を評価: a == 0 なら短絡
            prog.append(generate_expression(ctx, left)?);
            prog.push(Instruction::JumpIfZero(false_label));
            // 右辺を評価: b == 0 なら偽
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::JumpIfZero(false_label));
            // 両辺とも非0 → 真
            prog.push(Instruction::Push(WsNumber(1)));
            prog.push(Instruction::Jump(end_label));
            prog.push(Instruction::Label(false_label));
            prog.push(Instruction::Push(WsNumber(0)));
            prog.push(Instruction::Label(end_label));
        }
        // `a || b` のインライン展開:
        //   eval(a)
        //   JumpIfZero(check_b_label) # a == 0 なら b をチェック
        //   Push(1)                   # a != 0 → 短絡して真
        //   Jump(end_label)
        //   Label(check_b_label)
        //   eval(b)
        //   JumpIfZero(false_label)   # b == 0 なら偽
        //   Push(1)
        //   Jump(end_label)
        //   Label(false_label): Push(0)
        //   Label(end_label)
        Operator2::LogicalOr => {
            let check_b_label = ctx.new_label();
            let false_label = ctx.new_label();
            let end_label = ctx.new_label();
            // 左辺を評価: a == 0 なら b のチェックへ
            prog.append(generate_expression(ctx, left)?);
            prog.push(Instruction::JumpIfZero(check_b_label));
            // a != 0 → 短絡して真
            prog.push(Instruction::Push(WsNumber(1)));
            prog.push(Instruction::Jump(end_label));
            prog.push(Instruction::Label(check_b_label));
            // 右辺を評価: b == 0 なら偽
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::JumpIfZero(false_label));
            prog.push(Instruction::Push(WsNumber(1)));
            prog.push(Instruction::Jump(end_label));
            prog.push(Instruction::Label(false_label));
            prog.push(Instruction::Push(WsNumber(0)));
            prog.push(Instruction::Label(end_label));
        }

        // 代入
        Operator2::Assign => {
            // 左辺は変数参照、配列アクセス、またはデリファレンスである必要がある
            match &left.expression {
                ExecExpression::Variable(var_ref) => {
                    prog.append(generate_store_variable_impl(ctx, var_ref, right, true)?);
                }
                ExecExpression::ArrayAccess(var_ref, index_expr, _) => {
                    prog.append(generate_store_array_impl(
                        ctx, var_ref, index_expr, right, true,
                    )?);
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
                    return Err(make_error(
                        ctx,
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
///
/// `emit_retrieve` が `true` の場合、代入後に値を再取得してスタックに残す（value context）。
/// `false` の場合は Store のみで終了（void context）。
/// アドレス計算は `generate_variable_address` に委譲。
fn generate_store_variable_impl(
    ctx: &mut CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
    value_expr: &LocatedExecExpression,
    emit_retrieve: bool,
) -> Result<WsProgram, CompileError> {
    let mut prog = generate_variable_address(ctx, var_ref)?;
    prog.append(generate_expression(ctx, value_expr)?);
    prog.push(Instruction::Store);
    if emit_retrieve {
        // 代入式の値として value を残す（アドレスを再計算して Retrieve）
        prog.append(generate_variable_address(ctx, var_ref)?);
        prog.push(Instruction::Retrieve);
    }
    Ok(prog)
}

/// 配列要素への値の格納
/// arr[index] = value
///
/// `emit_retrieve` が `true` の場合、代入後に値を再取得してスタックに残す（value context）。
/// `false` の場合は Store のみで終了（void context）。
/// アドレス計算は `generate_array_element_address` に委譲。
fn generate_store_array_impl(
    ctx: &mut CodeGenContext,
    var_ref: &crate::semantic_analyzer::IdentifierRef,
    index_expr: &LocatedExecExpression,
    value_expr: &LocatedExecExpression,
    emit_retrieve: bool,
) -> Result<WsProgram, CompileError> {
    let mut prog = generate_array_element_address(ctx, var_ref, index_expr)?;
    prog.append(generate_expression(ctx, value_expr)?);
    prog.push(Instruction::Store);
    if emit_retrieve {
        // 代入式の値として value を残す（アドレスを再計算して Retrieve）
        prog.append(generate_array_element_address(ctx, var_ref, index_expr)?);
        prog.push(Instruction::Retrieve);
    }
    Ok(prog)
}

/// 関数呼び出し
fn generate_function_call(
    ctx: &mut CodeGenContext,
    kind: &crate::semantic_analyzer::BuiltinFunctionKind,
    args: &[Box<LocatedExecExpression>],
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
    args: &[Box<LocatedExecExpression>],
) -> Result<WsProgram, CompileError> {
    if args.len() != 1 {
        return Err(make_error(
            ctx,
            format!("__puti expects 1 argument, got {}", args.len()),
        ));
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
    args: &[Box<LocatedExecExpression>],
) -> Result<WsProgram, CompileError> {
    if args.len() != 1 {
        return Err(make_error(
            ctx,
            format!("__putc expects 1 argument, got {}", args.len()),
        ));
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
    ctx: &mut CodeGenContext,
    args: &[Box<LocatedExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !args.is_empty() {
        return Err(make_error(
            ctx,
            format!("__geti expects 0 arguments, got {}", args.len()),
        ));
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
    ctx: &mut CodeGenContext,
    args: &[Box<LocatedExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !args.is_empty() {
        return Err(make_error(
            ctx,
            format!("__getc expects 0 arguments, got {}", args.len()),
        ));
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
    args: &[Box<LocatedExecExpression>],
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
    args: &[Box<LocatedExecExpression>],
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
    args: &[Box<LocatedExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !ctx.is_alloc_ext() {
        return Err(make_error(
            ctx,
            "__alloc requires --std-ext alloc".to_string(),
        ));
    }
    if args.len() != 1 {
        return Err(make_error(
            ctx,
            format!("__alloc expects 1 argument, got {}", args.len()),
        ));
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
    args: &[Box<LocatedExecExpression>],
) -> Result<WsProgram, CompileError> {
    if !ctx.is_alloc_ext() {
        return Err(make_error(
            ctx,
            "__free requires --std-ext alloc".to_string(),
        ));
    }
    if args.len() != 1 {
        return Err(make_error(
            ctx,
            format!("__free expects 1 argument, got {}", args.len()),
        ));
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
    mode: &ConditionMode,
    cond: &LocatedExecExpression,
    then_block: &crate::semantic_analyzer::Block,
    else_block: &crate::semantic_analyzer::Block,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();

    let else_label = ctx.new_label();
    let end_label = ctx.new_label();

    // 条件評価
    prog.append(generate_expression(ctx, cond)?);

    // ConditionMode に応じたジャンプ命令
    match mode {
        ConditionMode::NonZero => {
            // cond == 0 (偽) なら else へジャンプ（既存動作）
            prog.push(Instruction::JumpIfZero(else_label));
        }
        ConditionMode::Zero => {
            // cond == 0 → then を実行 なので、cond != 0 なら else へジャンプ
            // JumpIfZero で then に落ちる、JumpIfNegative で else を飛ばす…
            // 実装: cond != 0 のときに else へジャンプ = cond == 0 なら then
            // Whitespace には JumpIfNotZero がないため、JumpIfZero で then_label に飛ばす方式
            let then_label = ctx.new_label();
            prog.push(Instruction::JumpIfZero(then_label));
            prog.push(Instruction::Jump(else_label));
            prog.push(Instruction::Label(then_label));
        }
        ConditionMode::Negative => {
            // cond < 0 → then を実行 なので、cond >= 0 なら else へジャンプ
            // Whitespace には JumpIfNonNegative がないため、JumpIfNegative で then_label に飛ばす
            let then_label = ctx.new_label();
            prog.push(Instruction::JumpIfNegative(then_label));
            prog.push(Instruction::Jump(else_label));
            prog.push(Instruction::Label(then_label));
        }
    }

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

/// 最適化パスで生成される内部組み込み関数のコード生成
///
/// InternalBuiltinFunction は最適化パスでのみ生成される。
/// 通常の組み込み関数と異なり、変数への直接格納など最適化された命令列を生成する。
fn generate_internal_builtin_function(
    ctx: &CodeGenContext,
    kind: &InternalBuiltinFunctionKind,
) -> Result<WsProgram, CompileError> {
    match kind {
        InternalBuiltinFunctionKind::Getiv(var_ref) => {
            // 変数アドレスに直接 InputNumber し、値をスタックに残す
            // 通常の __geti() は TEMP_PTR 経由だが、これは変数に直接格納する
            let mut prog = generate_variable_address(ctx, var_ref)?;
            prog.push(Instruction::Duplicate);
            prog.push(Instruction::InputNumber);
            prog.push(Instruction::Retrieve);
            Ok(prog)
        }
        InternalBuiltinFunctionKind::Getcv(var_ref) => {
            // 変数アドレスに直接 InputChar し、値をスタックに残す
            let mut prog = generate_variable_address(ctx, var_ref)?;
            prog.push(Instruction::Duplicate);
            prog.push(Instruction::InputChar);
            prog.push(Instruction::Retrieve);
            Ok(prog)
        }
    }
}

/// 式文のコード生成（discard-assign-value 最適化）
///
/// 代入式 `x = expr;` では、代入後の値再取得（Retrieve）をスキップする。
/// 代入式以外では通常の式評価 + Discard を行う。
///
/// これにより以下の命令を削減できる:
/// - グローバル変数代入: Push + Retrieve + Discard → 3命令削減
/// - ローカル変数代入: Push+Push+Retrieve+Add+Retrieve + Discard → 6命令削減
/// - 連鎖代入 `x = y = 5;` では外側のみ void context、内側は value context のまま
pub(crate) fn generate_expression_as_statement(
    ctx: &mut CodeGenContext,
    located_expr: &LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    ctx.set_location(&located_expr.location);
    match &located_expr.expression {
        ExecExpression::Operation2(Operator2::Assign, left, right) => {
            // 代入式: void コンテキストで生成（Retrieve をスキップし、Discard も不要）
            generate_assign_void(ctx, left, right)
        }
        _ => {
            // 代入式以外: 通常の式評価 + Discard
            let mut prog = generate_expression(ctx, located_expr)?;
            prog.push(Instruction::Discard);
            Ok(prog)
        }
    }
}

/// 代入式の void コンテキスト生成
/// Store のみ実施し、値再取得（Retrieve）をスキップする
fn generate_assign_void(
    ctx: &mut CodeGenContext,
    left: &LocatedExecExpression,
    right: &LocatedExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    match &left.expression {
        ExecExpression::Variable(var_ref) => {
            prog.append(generate_store_variable_impl(ctx, var_ref, right, false)?);
        }
        ExecExpression::ArrayAccess(var_ref, index_expr, _) => {
            prog.append(generate_store_array_impl(
                ctx, var_ref, index_expr, right, false,
            )?);
        }
        ExecExpression::Operation1(Operator1::Deref, addr_expr) => {
            // デリファレンス代入: *ptr = value（Retrieve なし）
            prog.append(generate_expression(ctx, addr_expr)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Store);
        }
        _ => {
            // 非代入左辺: 通常の代入生成 + Discard にフォールバック
            prog.append(generate_expression(ctx, left)?); // エラーになるはず
            prog.push(Instruction::Discard);
        }
    }
    Ok(prog)
}
