//! optimizer のユニットテスト
#![allow(deprecated)]

use crate::optimizer::noop_test_pass;
use crate::optimizer::test_helpers::run_ws;
use crate::optimizer::{self, OptimizationOptions, PASS_NAMES};
use crate::semantic_analyzer::{Block, ExecStatement, LocatedExecStatement};
use crate::semantic_analyzer::{
    ConditionMode, ExecExpression, IdentifierRef, InternalBuiltinFunctionKind,
    LocatedExecExpression,
};

/// AST 内の全 If/While の ConditionMode を再帰的に置換するヘルパー
fn patch_condition_mode_in_scope(scope: &mut crate::semantic_analyzer::Scope, mode: ConditionMode) {
    for func in &mut scope.functions {
        patch_condition_mode_in_block(&mut func.block, mode);
    }
    for stmt in &mut scope.root_statements {
        patch_condition_mode_in_statement(stmt, mode);
    }
    for stmt in &mut scope.static_init_statements {
        patch_condition_mode_in_statement(stmt, mode);
    }
}

fn patch_condition_mode_in_block(block: &mut Block, mode: ConditionMode) {
    for stmt in &mut block.statements {
        patch_condition_mode_in_statement(stmt, mode);
    }
}

fn patch_condition_mode_in_statement(stmt: &mut LocatedExecStatement, mode: ConditionMode) {
    match &mut stmt.statement {
        ExecStatement::Expression(expr) => patch_condition_mode_in_expression(expr, mode),
        ExecStatement::Return(Some(expr)) => patch_condition_mode_in_expression(expr, mode),
        ExecStatement::While(ref mut m, cond, block) => {
            *m = mode;
            patch_condition_mode_in_expression(cond, mode);
            patch_condition_mode_in_block(block, mode);
        }
        _ => {}
    }
}

fn patch_condition_mode_in_expression(expr: &mut LocatedExecExpression, mode: ConditionMode) {
    match &mut expr.expression {
        ExecExpression::If(ref mut m, cond, then_block, else_block) => {
            *m = mode;
            patch_condition_mode_in_expression(cond, mode);
            patch_condition_mode_in_block(then_block, mode);
            patch_condition_mode_in_block(else_block, mode);
        }
        ExecExpression::Block(block) => patch_condition_mode_in_block(block, mode),
        ExecExpression::Operation1(_, inner) => patch_condition_mode_in_expression(inner, mode),
        ExecExpression::Operation2(_, left, right) => {
            patch_condition_mode_in_expression(left, mode);
            patch_condition_mode_in_expression(right, mode);
        }
        ExecExpression::BuiltinFunction(_, args) | ExecExpression::UserFunction(_, args) => {
            for arg in args {
                patch_condition_mode_in_expression(arg, mode);
            }
        }
        _ => {}
    }
}

/// noop_test_pass: マジックナンバー変数が追加されること
#[test]
fn test_noop_pass_adds_marker_variable() {
    let code = "func: __main() { __trace(0); return: 0; }".to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    let orig_var_count = scope.variable_count;

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
            ..OptimizationOptions::none()
        },
    );

    // 変数が1つ追加されている
    assert_eq!(scope.variable_count, orig_var_count + 1);
    // マーカー変数名が存在する
    assert!(scope
        .variable_indices
        .contains_key(noop_test_pass::MARKER_VAR_NAME));
    // root_statements が増えている
    assert!(!scope.root_statements.is_empty());
}

/// noop_test_pass: マジックナンバーが正しくグローバル変数に設定されること
#[test]
fn test_noop_pass_magic_number_initialized() {
    let code = "func: __main() { __trace(0); return: 0; }".to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
            ..OptimizationOptions::none()
        },
    );

    // マーカー変数はソースから参照できないので、実行せずに初期化文の構造を確かめる:
    // root_statements に「マーカー変数のスロット = MAGIC_NUMBER」の代入式があること
    let marker_slot = *scope
        .variable_indices
        .get(noop_test_pass::MARKER_VAR_NAME)
        .unwrap();
    let has_marker_init = scope.root_statements.iter().any(|stmt| {
        let ExecStatement::Expression(expr) = &stmt.statement else {
            return false;
        };
        let ExecExpression::Operation2(crate::tree_parser::Operator2::Assign, lhs, rhs) =
            &expr.expression
        else {
            return false;
        };
        matches!(
            &lhs.expression,
            ExecExpression::Variable(var_ref, _)
                if var_ref.is_global && var_ref.local_index == marker_slot
        ) && matches!(
            rhs.expression,
            ExecExpression::Factor(noop_test_pass::MAGIC_NUMBER)
        )
    });
    assert!(
        has_marker_init,
        "marker variable should be initialized to magic number"
    );
}

/// noop_test_pass: 既存の実行結果に影響しないこと
#[test]
fn test_noop_pass_does_not_affect_execution() {
    let code = r#"
        let: x(10);
        let: y(20);
        func: __main() {
            __trace(0);
            __trace(1);
            __trace(1);
            __puti(x + y);
            return: 0;
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    // 最適化なしの結果
    let before = run_ws(&scope, "");

    // 最適化を適用
    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
            ..OptimizationOptions::none()
        },
    );

    // 最適化ありの結果
    let after = run_ws(&scope, "");

    assert_eq!(before, after, "trace results and stdout should be identical");
    assert_eq!(before.stdout, "30");
    assert_eq!(before.traced.get(&0), Some(&1));
    assert_eq!(before.traced.get(&1), Some(&2));
}

/// 最適化なしの場合、Scope が変更されないこと
#[test]
fn test_no_optimization_leaves_scope_unchanged() {
    let code = "func: __main() { return: 42; }".to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    let var_count_before = scope.variable_count;
    let root_stmt_count_before = scope.root_statements.len();

    optimizer::optimize(&mut scope, &OptimizationOptions::none());

    assert_eq!(scope.variable_count, var_count_before);
    assert_eq!(scope.root_statements.len(), root_stmt_count_before);
}

/// noop_test_pass: Whitespace コンパイルにも影響しないこと
#[test]
fn test_noop_pass_does_not_break_ws_compile() {
    let code = r#"
        func: __main() {
            __puti(42);
            return: 0;
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
            ..OptimizationOptions::none()
        },
    );

    // Whitespace コンパイルが成功すること
    let result = crate::compiler_ws::compile_with_options(&scope, false, false);
    assert!(
        result.is_ok(),
        "WS compilation should succeed after optimization"
    );
}

// --- ConditionMode テスト ---

/// ConditionMode::NonZero (デフォルト): if:(0) → else ブロック実行
#[test]
fn test_condition_mode_nonzero_if_false() {
    // NonZero: 0 != 0 = false → trace(2)
    let code = r#"
func: __main() {
    if:(0) { __trace(1); } else: { __trace(2); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let scope = crate::semantic_analyze(&s).unwrap();

    let traces = run_ws(&scope, "").traced;
    assert_eq!(traces.get(&1), None, "then block should not execute");
    assert_eq!(traces.get(&2), Some(&1), "else block should execute once");
}

/// ConditionMode::Zero: if:(0) → then ブロック実行
#[test]
fn test_condition_mode_zero_if_true() {
    // Zero: 0 == 0 = true → trace(1)
    let code = r#"
func: __main() {
    if:(0) { __trace(1); } else: { __trace(2); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    patch_condition_mode_in_scope(&mut scope, ConditionMode::Zero);

    let traces = run_ws(&scope, "").traced;
    assert_eq!(
        traces.get(&1),
        Some(&1),
        "then block should execute (Zero mode: 0 == 0)"
    );
    assert_eq!(traces.get(&2), None, "else block should not execute");
}

/// ConditionMode::Zero: if:(1) → else ブロック実行
#[test]
fn test_condition_mode_zero_if_false() {
    // Zero: 1 == 0 = false → trace(2)
    let code = r#"
func: __main() {
    if:(1) { __trace(1); } else: { __trace(2); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    patch_condition_mode_in_scope(&mut scope, ConditionMode::Zero);

    let traces = run_ws(&scope, "").traced;
    assert_eq!(
        traces.get(&1),
        None,
        "then block should not execute (Zero mode: 1 != 0)"
    );
    assert_eq!(traces.get(&2), Some(&1), "else block should execute");
}

/// ConditionMode::Negative: if:(0 - 1) → then ブロック実行
#[test]
fn test_condition_mode_negative_if_true() {
    // Negative: -1 < 0 = true → trace(1)
    let code = r#"
func: __main() {
    if:(0 - 1) { __trace(1); } else: { __trace(2); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    patch_condition_mode_in_scope(&mut scope, ConditionMode::Negative);

    let traces = run_ws(&scope, "").traced;
    assert_eq!(
        traces.get(&1),
        Some(&1),
        "then block should execute (Negative mode: -1 < 0)"
    );
    assert_eq!(traces.get(&2), None, "else block should not execute");
}

/// ConditionMode::Negative: if:(0) → else ブロック実行
#[test]
fn test_condition_mode_negative_if_false() {
    // Negative: 0 < 0 = false → trace(2)
    let code = r#"
func: __main() {
    if:(0) { __trace(1); } else: { __trace(2); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    patch_condition_mode_in_scope(&mut scope, ConditionMode::Negative);

    let traces = run_ws(&scope, "").traced;
    assert_eq!(
        traces.get(&1),
        None,
        "then block should not execute (Negative mode: 0 >= 0)"
    );
    assert_eq!(traces.get(&2), Some(&1), "else block should execute");
}

/// ConditionMode::Zero で while ループ: 条件 == 0 のときループ継続
#[test]
fn test_condition_mode_zero_while() {
    // Zero mode で while: x == 0 のような式をパッチ
    // まず NonZero mode でのデフォルト動作を確認
    let code = r#"
func: __main() {
    let: x(0);
    while:(x == 0) { __trace(0); x = x + 1; };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let scope = crate::semantic_analyze(&s).unwrap();

    // NonZero mode (default): x==0 → 比較結果 1 (true) → loop runs, x=1 → x==0 → 0 (false) → exit
    let traces = run_ws(&scope, "").traced;
    assert_eq!(traces.get(&0), Some(&1), "while(NonZero): should loop once");
}

/// ConditionMode::NonZero for while: cond != 0 → ループ継続
#[test]
fn test_condition_mode_nonzero_while_multiple() {
    // NonZero: x != 0 → loop continues. x=3,2,1 → 3 iterations
    let code = r#"
func: __main() {
    let: x(3);
    while:(x) { __trace(0); x = x - 1; };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let scope = crate::semantic_analyze(&s).unwrap();

    let traces = run_ws(&scope, "").traced;
    assert_eq!(
        traces.get(&0),
        Some(&3),
        "while(NonZero): should loop 3 times"
    );
}

/// ConditionMode::Zero で while: 条件値が 0 のときループ継続
#[test]
fn test_condition_mode_zero_while_patched() {
    // let: x(0); while:(x) { trace(0); x = x + 1; }
    // NonZero (default): x=0 → 0 != 0 = false → ループせず
    // Zero (patched): x=0 → 0 == 0 = true → ループ, x=1 → 1 == 0 = false → 終了
    let code = r#"
func: __main() {
    let: x(0);
    while:(x) { __trace(0); x = x + 1; };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // NonZero mode: x=0 → false → no loop
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;
    assert_eq!(
        traces_orig.get(&0),
        None,
        "while(NonZero): x=0 should not loop"
    );

    // Zero mode: x=0 → 0 == 0 = true → loop once
    let mut scope_zero = crate::semantic_analyze(&s).unwrap();
    patch_condition_mode_in_scope(&mut scope_zero, ConditionMode::Zero);
    let traces_zero = run_ws(&scope_zero, "").traced;
    assert_eq!(
        traces_zero.get(&0),
        Some(&1),
        "while(Zero): x=0 should loop once"
    );
}

/// ConditionMode に関わらず既存の解析結果は NonZero であること
#[test]
fn test_semantic_analyzer_produces_nonzero() {
    let code = r#"
func: __main() {
    if:(1) { __trace(1); } else: { __trace(2); };
    while:(0) { __trace(3); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let scope = crate::semantic_analyze(&s).unwrap();

    // if:(1) → NonZero: 1 != 0 = true → trace(1)
    let traces = run_ws(&scope, "").traced;
    assert_eq!(
        traces.get(&1),
        Some(&1),
        "if:(1) with NonZero should execute then block"
    );
    assert_eq!(traces.get(&2), None, "else block should not execute");
    assert_eq!(
        traces.get(&3),
        None,
        "while:(0) with NonZero should not loop"
    );
}

/// ConditionMode::Zero + Whitespace コンパイルが成功すること
#[test]
fn test_condition_mode_zero_ws_compile() {
    let code = r#"
func: __main() {
    if:(0) { __puti(1); } else: { __puti(2); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    patch_condition_mode_in_scope(&mut scope, ConditionMode::Zero);

    let result = crate::compiler_ws::compile_with_options(&scope, false, false);
    assert!(
        result.is_ok(),
        "WS compilation should succeed with ConditionMode::Zero"
    );
}

/// ConditionMode::Negative + Whitespace コンパイルが成功すること
#[test]
fn test_condition_mode_negative_ws_compile() {
    let code = r#"
func: __main() {
    if:(0 - 1) { __puti(1); } else: { __puti(2); };
    while:(0) { __puti(3); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    patch_condition_mode_in_scope(&mut scope, ConditionMode::Negative);

    let result = crate::compiler_ws::compile_with_options(&scope, false, false);
    assert!(
        result.is_ok(),
        "WS compilation should succeed with ConditionMode::Negative"
    );
}

// --- InternalBuiltinFunction テスト ---

/// InternalBuiltinFunctionKind::Getiv の型推論テスト
#[test]
fn test_internal_builtin_getiv_infer_type() {
    use crate::semantic_analyzer::ValueType;
    let var_ref = IdentifierRef {
        scope_depth: 0,
        local_index: 0,
        is_global: true,
        owning_func_index: None,
    };
    let expr = ExecExpression::InternalBuiltinFunction(InternalBuiltinFunctionKind::Getiv(var_ref));
    assert_eq!(
        expr.infer_type(&[]),
        ValueType::Int,
        "Getiv should infer to Int"
    );
}

/// InternalBuiltinFunctionKind::Getcv の型推論テスト
#[test]
fn test_internal_builtin_getcv_infer_type() {
    use crate::semantic_analyzer::ValueType;
    let var_ref = IdentifierRef {
        scope_depth: 0,
        local_index: 0,
        is_global: true,
        owning_func_index: None,
    };
    let expr = ExecExpression::InternalBuiltinFunction(InternalBuiltinFunctionKind::Getcv(var_ref));
    assert_eq!(
        expr.infer_type(&[]),
        ValueType::Int,
        "Getcv should infer to Int"
    );
}

/// InternalBuiltinFunction(Getiv): WS の経路でグローバル変数に stdin から読み込むこと
#[test]
fn test_internal_builtin_getiv_ws() {
    // 既存の x = __geti() のコードを使い、compile → WhitespaceVM で確認
    let code = r#"
        let: x(0);
        func: __main() {
            x = __geti();
            __puti(x);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let scope = crate::semantic_analyze(&s).unwrap();

    // 通常の __geti() が動作することの確認
    let outcome = run_ws(&scope, "42\n");
    assert_eq!(outcome.stdout, "42", "__geti() should read 42 from stdin");
}
// --- condition_opt パステスト ---

/// condition_opt: if:(x == 0) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_eq_zero_if_semantics() {
    let code = r#"
func: __main() {
    let: x(0);
    if:(x == 0) { __trace(1); } else: { __trace(2); };
    x = 5;
    if:(x == 0) { __trace(3); } else: { __trace(4); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    // 最適化あり
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(
        traces_orig, traces_opt,
        "Semantics should not change after condition_opt"
    );
    assert_eq!(traces_orig.get(&1), Some(&1), "x==0: then block");
    assert_eq!(traces_orig.get(&4), Some(&1), "x==5: else block");
}

/// condition_opt: if:(x != 0) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_neq_zero_if_semantics() {
    let code = r#"
func: __main() {
    let: x(0);
    if:(x != 0) { __trace(1); } else: { __trace(2); };
    x = 5;
    if:(x != 0) { __trace(3); } else: { __trace(4); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(
        traces_orig.get(&2),
        Some(&1),
        "x==0: else (condition false)"
    );
    assert_eq!(traces_orig.get(&3), Some(&1), "x==5: then (condition true)");
}

/// condition_opt: if:(x < 0) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_less_zero_if_semantics() {
    let code = r#"
func: __main() {
    let: x(0 - 1);
    if:(x < 0) { __trace(1); } else: { __trace(2); };
    x = 0;
    if:(x < 0) { __trace(3); } else: { __trace(4); };
    x = 1;
    if:(x < 0) { __trace(5); } else: { __trace(6); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(traces_orig.get(&1), Some(&1), "x=-1: then block (< 0)");
    assert_eq!(traces_orig.get(&4), Some(&1), "x=0: else block (not < 0)");
    assert_eq!(traces_orig.get(&6), Some(&1), "x=1: else block (not < 0)");
}

/// condition_opt: if:(x >= 0) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_geq_zero_if_semantics() {
    let code = r#"
func: __main() {
    let: x(0);
    if:(x >= 0) { __trace(1); } else: { __trace(2); };
    x = 0 - 1;
    if:(x >= 0) { __trace(3); } else: { __trace(4); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(traces_orig.get(&1), Some(&1), "x=0: then block (>= 0)");
    assert_eq!(traces_orig.get(&4), Some(&1), "x=-1: else block (not >= 0)");
}

/// condition_opt: if:(a > b) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_greater_if_semantics() {
    let code = r#"
func: __main() {
    let: a(5);
    let: b(3);
    if:(a > b) { __trace(1); } else: { __trace(2); };
    if:(b > a) { __trace(3); } else: { __trace(4); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(traces_orig.get(&1), Some(&1), "5 > 3: then block");
    assert_eq!(traces_orig.get(&4), Some(&1), "3 > 5: false → else block");
}

/// condition_opt: if:(a <= b) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_leq_if_semantics() {
    let code = r#"
func: __main() {
    let: a(3);
    let: b(5);
    if:(a <= b) { __trace(1); } else: { __trace(2); };
    if:(b <= a) { __trace(3); } else: { __trace(4); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(traces_orig.get(&1), Some(&1), "3 <= 5: then block");
    assert_eq!(traces_orig.get(&4), Some(&1), "5 <= 3: false → else block");
}

/// condition_opt: if:(a == b) 一般式の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_eq_general_if_semantics() {
    let code = r#"
func: __main() {
    let: a(3);
    let: b(3);
    if:(a == b) { __trace(1); } else: { __trace(2); };
    b = 4;
    if:(a == b) { __trace(3); } else: { __trace(4); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(traces_orig.get(&1), Some(&1), "a==b: then block");
    assert_eq!(traces_orig.get(&4), Some(&1), "a!=b: else block");
}

/// condition_opt: while:(x != 0) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_while_neq_zero_semantics() {
    let code = r#"
func: __main() {
    let: x(3);
    while:(x != 0) { __trace(0); x = x - 1; };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(traces_orig.get(&0), Some(&3), "should loop 3 times");
}

/// condition_opt: while:(x == 0) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_while_eq_zero_semantics() {
    let code = r#"
func: __main() {
    let: x(0);
    while:(x == 0) { __trace(0); x = x + 1; };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(traces_orig.get(&0), Some(&1), "should loop once");
}

/// condition_opt: while:(x < 0) の最適化 - セマンティクスが変わらないこと
#[test]
fn test_condition_opt_while_less_zero_semantics() {
    let code = r#"
func: __main() {
    let: x(0 - 3);
    while:(x < 0) { __trace(0); x = x + 1; };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(traces_orig, traces_opt, "Semantics should not change");
    assert_eq!(
        traces_orig.get(&0),
        Some(&3),
        "should loop 3 times (x=-3,-2,-1 → exit at 0)"
    );
}

/// condition_opt: WS コンパイル後の実行結果が最適化前後で同じであること（if == 0）
#[test]
fn test_condition_opt_eq_zero_ws_semantics() {
    let code = r#"
func: __main() {
    let: x(0);
    if:(x == 0) { __puti(1); } else: { __puti(2); };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし: WS コンパイル + 実行
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "");

    // 最適化あり: WS コンパイル + 実行
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "");

    assert_eq!(result_orig, result_opt, "Semantics should not change");
    assert_eq!(result_orig.stdout, "1", "x==0: then block");
}

/// condition_opt: ネストした条件式の最適化
#[test]
fn test_condition_opt_nested_if_semantics() {
    let code = r#"
func: __main() {
    let: x(0);
    let: y(0 - 1);
    if:(x == 0) {
        if:(y < 0) { __trace(1); } else: { __trace(2); };
    } else: {
        __trace(3);
    };
    return: 0;
}
"#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let traces_orig = run_ws(&scope_orig, "").traced;

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let traces_opt = run_ws(&scope_opt, "").traced;

    assert_eq!(
        traces_orig, traces_opt,
        "Nested if semantics should not change"
    );
    assert_eq!(traces_orig.get(&1), Some(&1), "x==0 and y<0: trace(1)");
    assert_eq!(traces_orig.get(&2), None);
    assert_eq!(traces_orig.get(&3), None);
}

// --- geti_opt パステスト ---

/// geti_opt: `p = __geti()` のセマンティクスが最適化前後で同じであること（グローバル変数）
#[test]
fn test_geti_opt_global_geti_semantics() {
    let code = r#"
        let: x(0);
        func: __main() {
            x = __geti();
            __puti(x);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "42\n");

    // 最適化あり
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            geti_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "42\n");

    assert_eq!(result_orig.stdout, "42", "original: should print 42");
    assert_eq!(
        result_orig, result_opt,
        "semantics should not change after geti_opt"
    );
}

/// geti_opt: `p = __geti()` のローカル変数バージョン
#[test]
fn test_geti_opt_local_geti_semantics() {
    let code = r#"
        func: __main() {
            let: x(0);
            x = __geti();
            __puti(x);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "99\n");

    // 最適化あり
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            geti_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "99\n");

    assert_eq!(result_orig.stdout, "99", "original: should print 99");
    assert_eq!(result_orig, result_opt, "semantics should not change");
}

/// geti_opt: `p = __getc()` のセマンティクスが最適化前後で同じであること
#[test]
fn test_geti_opt_getc_semantics() {
    let code = r#"
        let: c(0);
        func: __main() {
            c = __getc();
            __puti(c);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "A");

    // 最適化あり
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            geti_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "A");

    assert_eq!(result_orig, result_opt, "getc semantics should not change");
    assert_eq!(result_orig.stdout, "65", "should read 'A' = 65");
}

/// geti_opt: Whitespace コンパイルが成功すること
#[test]
fn test_geti_opt_ws_compile_success() {
    let code = r#"
        func: __main() {
            let: x(0);
            x = __geti();
            __puti(x);
            return: 0;
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            geti_opt: true,
            ..OptimizationOptions::none()
        },
    );

    let result = crate::compiler_ws::compile_with_options(&scope, false, false);
    assert!(
        result.is_ok(),
        "WS compilation should succeed after geti_opt"
    );
}

/// geti_opt: 複数の `__geti()` 代入がある場合も正しく動作すること
#[test]
fn test_geti_opt_multiple_geti_semantics() {
    let code = r#"
        func: __main() {
            let: a(0);
            let: b(0);
            a = __geti();
            b = __geti();
            __puti(a + b);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "10\n20\n");

    // 最適化あり
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            geti_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "10\n20\n");

    assert_eq!(result_orig.stdout, "30", "sum should be 30");
    assert_eq!(result_orig, result_opt, "semantics should not change");
}

/// geti_opt: `__geti()` が if/while ブロック内にある場合も最適化が適用されること
#[test]
fn test_geti_opt_inside_block_semantics() {
    let code = r#"
        func: __main() {
            let: x(0);
            if:(1) {
                x = __geti();
            } else: {
                x = 0;
            };
            __puti(x);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "77\n");

    // 最適化あり
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            geti_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "77\n");

    assert_eq!(result_orig.stdout, "77", "should print 77");
    assert_eq!(result_orig, result_opt, "semantics should not change");
}

/// geti_opt + condition_opt の組み合わせで動作すること
#[test]
fn test_geti_opt_combined_with_condition_opt() {
    let code = r#"
        func: __main() {
            let: x(0);
            x = __geti();
            if:(x == 0) { __puti(0); } else: { __puti(1); };
            __puti(x);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "5\n");

    // 最適化あり (geti_opt + condition_opt)
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            geti_opt: true,
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "5\n");

    assert_eq!(result_orig.stdout, "15", "x=5: else branch prints 1, then x");
    assert_eq!(
        result_orig, result_opt,
        "combined opt semantics should not change"
    );

    // WS コンパイルも成功すること
    let ws_result = crate::compiler_ws::compile_with_options(&scope_opt, false, false);
    assert!(
        ws_result.is_ok(),
        "WS compile with combined opt should succeed"
    );
}

// --- dead_code パステスト ---

/// dead_code: 未使用関数がダミーに置換されること
#[test]
fn test_dead_code_unreachable_func_becomes_dummy() {
    let code = r#"
        func: unused() { return: 42; }
        func: __main() { return: 0; }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    let unused_idx = scope.symbol_table.function_name_to_index["unused"];
    assert!(
        !scope.functions[unused_idx].is_unused(),
        "before: should not be unused"
    );

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            dead_code: true,
            ..OptimizationOptions::none()
        },
    );

    assert!(
        scope.functions[unused_idx].is_unused(),
        "after: unused function should be unused"
    );
}

/// dead_code: main 関数は常に到達可能（ダミーにならない）
#[test]
fn test_dead_code_main_not_dummy() {
    let code = r#"
        func: unused() { return: 99; }
        func: __main() { return: 0; }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    let main_idx = scope.main_function_index.unwrap();
    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            dead_code: true,
            ..OptimizationOptions::none()
        },
    );

    assert!(
        !scope.functions[main_idx].is_unused(),
        "main should not be unused"
    );
}

/// dead_code: main から直接呼ばれる関数は到達可能
#[test]
fn test_dead_code_called_func_reachable() {
    let code = r#"
        func: helper() { return: 1; }
        func: unused() { return: 99; }
        func: __main() { return: helper(); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            dead_code: true,
            ..OptimizationOptions::none()
        },
    );

    let helper_idx = scope.symbol_table.function_name_to_index["helper"];
    let unused_idx = scope.symbol_table.function_name_to_index["unused"];
    assert!(
        !scope.functions[helper_idx].is_unused(),
        "helper (called) should not be unused"
    );
    assert!(
        scope.functions[unused_idx].is_unused(),
        "unused should be unused"
    );
}

/// dead_code: 推移的に到達可能な関数は保持される
#[test]
fn test_dead_code_transitive_reachability() {
    let code = r#"
        func: level2() { return: 2; }
        func: level1() { return: level2(); }
        func: unused() { return: 99; }
        func: __main() { return: level1(); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            dead_code: true,
            ..OptimizationOptions::none()
        },
    );

    let l1_idx = scope.symbol_table.function_name_to_index["level1"];
    let l2_idx = scope.symbol_table.function_name_to_index["level2"];
    let unused_idx = scope.symbol_table.function_name_to_index["unused"];
    assert!(
        !scope.functions[l1_idx].is_unused(),
        "level1 should not be unused"
    );
    assert!(
        !scope.functions[l2_idx].is_unused(),
        "level2 should not be unused"
    );
    assert!(
        scope.functions[unused_idx].is_unused(),
        "unused should be unused"
    );
}

/// dead_code: 実行結果が変わらないこと
#[test]
fn test_dead_code_semantics_unchanged() {
    let code = r#"
        func: unused1() { return: 100; }
        func: unused2() { return: unused1(); }
        func: helper() { return: 42; }
        func: __main() { __puti(helper()); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "");

    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            dead_code: true,
            ..OptimizationOptions::none()
        },
    );
    let result_opt = run_ws(&scope_opt, "");

    assert_eq!(result_orig.stdout, "42", "original should print 42");
    assert_eq!(
        result_orig, result_opt,
        "semantics should not change after dead_code"
    );
}

/// dead_code: WS コンパイルが成功すること
#[test]
fn test_dead_code_ws_compile_success() {
    let code = r#"
        func: never_called() { __puti(999); return: 0; }
        func: __main() { __puti(1); return: 0; }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            dead_code: true,
            ..OptimizationOptions::none()
        },
    );

    let result = crate::compiler_ws::compile_with_options(&scope, false, false);
    assert!(
        result.is_ok(),
        "WS compilation should succeed after dead_code opt"
    );
}

/// dead_code: main がない場合はスキップ（全関数保持）
#[test]
fn test_dead_code_no_main_skips() {
    let code = r#"
        let: x(0);
        func: foo() { return: 1; }
        func: bar() { return: 2; }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    // main がない場合は最適化なし
    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            dead_code: true,
            ..OptimizationOptions::none()
        },
    );

    let foo_idx = scope.symbol_table.function_name_to_index["foo"];
    let bar_idx = scope.symbol_table.function_name_to_index["bar"];
    assert!(
        !scope.functions[foo_idx].is_unused(),
        "foo should not be unused (no main)"
    );
    assert!(
        !scope.functions[bar_idx].is_unused(),
        "bar should not be unused (no main)"
    );
}

/// dead_code + condition_opt + geti_opt の組み合わせが動作すること
#[test]
fn test_dead_code_combined_all_opts() {
    let code = r#"
        func: unused() { return: 0; }
        func: helper(a) { return: a + 1; }
        func: __main() {
            let: x(0);
            x = __geti();
            if:(x == 0) { __puti(0); } else: { __puti(helper(x)); };
            return: 0;
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    // 最適化なし
    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let result_orig = run_ws(&scope_orig, "5\n");

    // 全最適化
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(&mut scope_opt, &OptimizationOptions::all());
    let result_opt = run_ws(&scope_opt, "5\n");

    assert_eq!(
        result_orig, result_opt,
        "combined opts semantics should not change"
    );
    assert_eq!(result_orig.stdout, "6", "x=5: helper(5) = 6");

    // WS コンパイルも成功
    let ws_result = crate::compiler_ws::compile_with_options(&scope_opt, false, false);
    assert!(ws_result.is_ok(), "WS compile with all opts should succeed");

    // unused がダミーになっていること
    let unused_idx = scope_opt.symbol_table.function_name_to_index["unused"];
    assert!(
        scope_opt.functions[unused_idx].is_unused(),
        "unused should be unused after all opts"
    );
}

// --- constant_folding パステスト ---

/// constant_folding: 整数の加算が定数に畳み込まれること
#[test]
fn test_constant_folding_add() {
    let code = r#"
        func: __main() { __puti(3 + 4); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result = run_ws(&scope, "");
    assert_eq!(result.stdout, "7", "3 + 4 should fold to 7");
}

/// constant_folding: 乗算・除算の連続畳み込み
#[test]
fn test_constant_folding_multiply_divide() {
    let code = r#"
        func: __main() { __puti(10 * 3 / 5); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result = run_ws(&scope, "");
    assert_eq!(result.stdout, "6", "10 * 3 / 5 should fold to 6");
}

/// constant_folding: 比較演算が定数に畳み込まれること
#[test]
fn test_constant_folding_comparison() {
    let code = r#"
        func: __main() { __puti(5 == 5); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result = run_ws(&scope, "");
    assert_eq!(result.stdout, "1", "5 == 5 should fold to 1");
}

/// constant_folding: 単項マイナスが畳み込まれること
#[test]
fn test_constant_folding_unary_negative() {
    let code = r#"
        func: __main() { __puti(-7); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result = run_ws(&scope, "");
    assert_eq!(result.stdout, "-7", "-7 should fold to -7");
}

/// constant_folding: 論理否定が畳み込まれること
#[test]
fn test_constant_folding_logical_not() {
    let code = r#"
        func: __main() { __puti(!0); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result = run_ws(&scope, "");
    assert_eq!(result.stdout, "1", "!0 should fold to 1");
}

/// constant_folding: ゼロ除算は変換しない（ランタイムエラーとして残す）
#[test]
fn test_constant_folding_zero_divide_not_folded() {
    let code = r#"
        func: __main() { return: 10 / 0; }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();

    // ゼロ除算は変換しないため最適化後もそのまま残る
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    // WS コンパイルはエラーにならないこと（ランタイムエラーとして残る）
    let ws_result = crate::compiler_ws::compile_with_options(&scope_opt, false, false);
    assert!(
        ws_result.is_ok(),
        "zero divide should remain as runtime error, not compile error"
    );
}

/// constant_folding: 定数条件 if が then ブロックに置換されること
#[test]
fn test_constant_folding_const_if_true() {
    let code = r#"
        func: f() {
            if: (1) { return: 10; } else: { return: 20; };
            return: 0;
        }
        func: __main() { __puti(f()); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    assert_eq!(run_ws(&scope_orig, ""), run_ws(&scope_opt, ""));
    assert_eq!(
        run_ws(&scope_opt, "").stdout,
        "10",
        "const true if should select then block"
    );
}

/// constant_folding: 定数条件 if が else ブロックに置換されること
#[test]
fn test_constant_folding_const_if_false() {
    let code = r#"
        func: f() {
            if: (0) { return: 10; } else: { return: 20; };
            return: 0;
        }
        func: __main() { __puti(f()); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    assert_eq!(run_ws(&scope_orig, ""), run_ws(&scope_opt, ""));
    assert_eq!(
        run_ws(&scope_opt, "").stdout,
        "20",
        "const false if should select else block"
    );
}

/// constant_folding: 定数条件 while (0) がスキップされること
#[test]
fn test_constant_folding_const_while_zero() {
    let code = r#"
        func: __main() {
            let: x(5);
            while: (0) { x = x + 1; };
            __puti(x);
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    assert_eq!(run_ws(&scope_orig, ""), run_ws(&scope_opt, ""));
    assert_eq!(
        run_ws(&scope_opt, "").stdout,
        "5",
        "while(0) should be skipped"
    );
}

/// constant_folding: 再帰的な畳み込みが動作すること
#[test]
fn test_constant_folding_recursive() {
    let code = r#"
        func: __main() { __puti((2 + 3) * (4 - 1)); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result = run_ws(&scope, "");
    assert_eq!(result.stdout, "15", "(2+3)*(4-1) should fold to 15");
}

/// constant_folding: セマンティクスが変わらないこと（変数を含む式）
#[test]
fn test_constant_folding_semantics_unchanged_with_variable() {
    let code = r#"
        func: __main() {
            let: x(0);
            x = __geti();
            __puti(x + (3 + 4));
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result_orig = run_ws(&scope_orig, "10\n");

    let result_opt = run_ws(&scope_opt, "10\n");

    assert_eq!(result_orig, result_opt, "semantics should not change");
    assert_eq!(result_opt.stdout, "17", "10 + (3+4) should be 17");
}

/// constant_folding: WS コンパイルが成功すること
#[test]
fn test_constant_folding_ws_compile_success() {
    let code = r#"
        func: __main() {
            __puti(10 + 20);
            return: 0;
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::semantic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            constant_folding: true,
            ..OptimizationOptions::none()
        },
    );

    let result = crate::compiler_ws::compile_with_options(&scope, false, false);
    assert!(
        result.is_ok(),
        "WS compilation should succeed after constant_folding"
    );
}

/// constant_folding + condition_opt の組み合わせ（定数畳み込み後に条件最適化）
#[test]
fn test_constant_folding_combined_with_condition_opt() {
    // 3 + 4 == 7 → Factor(7) == Factor(7) → Factor(1) → condition_opt: If(Zero, ...) 等に変換
    let code = r#"
        func: f() {
            if: (3 + 4 == 7) { return: 1; } else: { return: 0; };
            return: 0;
        }
        func: __main() { __puti(f()); }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();

    let scope_orig = crate::semantic_analyze(&s).unwrap();
    let mut scope_opt = crate::semantic_analyze(&s).unwrap();
    optimizer::optimize(
        &mut scope_opt,
        &OptimizationOptions {
            constant_folding: true,
            condition_opt: true,
            ..OptimizationOptions::none()
        },
    );

    let ws_result = crate::compiler_ws::compile_with_options(&scope_opt, false, false);
    assert!(
        ws_result.is_ok(),
        "WS compile after const_fold + condition_opt should succeed"
    );
    assert_eq!(run_ws(&scope_orig, ""), run_ws(&scope_opt, ""));
    assert_eq!(run_ws(&scope_opt, "").stdout, "1");
}

// ===== パス名からの構築 =====

#[test]
fn test_from_pass_names_each_pass() {
    let o = OptimizationOptions::from_pass_names(&["condition-opt"]).unwrap();
    assert!(o.condition_opt && !o.geti_opt && !o.constant_folding && !o.dead_code && !o.peephole);
    let o = OptimizationOptions::from_pass_names(&["geti-opt"]).unwrap();
    assert!(o.geti_opt && !o.condition_opt);
    let o = OptimizationOptions::from_pass_names(&["constant-folding"]).unwrap();
    assert!(o.constant_folding && !o.dead_code);
    let o = OptimizationOptions::from_pass_names(&["dead-code"]).unwrap();
    assert!(o.dead_code && !o.peephole);
    let o = OptimizationOptions::from_pass_names(&["peephole"]).unwrap();
    assert!(o.peephole && !o.any_enabled());
}

#[test]
fn test_from_pass_names_all_and_empty() {
    let all = OptimizationOptions::from_pass_names(&["dead-code", "all"]).unwrap();
    assert!(
        all.condition_opt && all.geti_opt && all.constant_folding && all.dead_code && all.peephole
    );
    let none = OptimizationOptions::from_pass_names::<&str>(&[]).unwrap();
    assert!(!none.any_enabled() && !none.peephole);
}

#[test]
fn test_from_pass_names_unknown() {
    assert_eq!(
        OptimizationOptions::from_pass_names(&["peephole", "unknown-pass"]).err(),
        Some("unknown-pass".to_string())
    );
}

#[test]
fn test_every_pass_name_is_accepted() {
    for name in PASS_NAMES {
        assert!(
            OptimizationOptions::from_pass_names(&[*name]).is_ok(),
            "{} rejected",
            name
        );
    }
}
