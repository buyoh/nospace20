//! optimizer のユニットテスト

use crate::optimizer::{self, OptimizationOptions};
use crate::optimizer::noop_test_pass;

/// noop_test_pass: マジックナンバー変数が追加されること
#[test]
fn test_noop_pass_adds_marker_variable() {
    let code = "func: main() { __trace(0); return: 0; }".to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::syntactic_analyze(&s).unwrap();

    let orig_var_count = scope.variable_count;

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
        },
    );

    // 変数が1つ追加されている
    assert_eq!(scope.variable_count, orig_var_count + 1);
    // マーカー変数名が存在する
    assert!(scope.variable_indices.contains_key(noop_test_pass::MARKER_VAR_NAME));
    // root_statements が増えている
    assert!(!scope.root_statements.is_empty());
}

/// noop_test_pass: マジックナンバーが正しくグローバル変数に設定されること
#[test]
fn test_noop_pass_magic_number_initialized() {
    let code = "func: main() { __trace(0); return: 0; }".to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::syntactic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
        },
    );

    // interpret して、グローバル変数にマジックナンバーが設定されていることを確認
    let mut env = crate::Environment::new();
    crate::interpret_with_env(&mut env, &scope);

    let marker_slot = *scope
        .variable_indices
        .get(noop_test_pass::MARKER_VAR_NAME)
        .unwrap();
    assert_eq!(
        env.global_variables[marker_slot],
        noop_test_pass::MAGIC_NUMBER,
        "marker variable should be initialized to magic number"
    );
}

/// noop_test_pass: 既存の実行結果に影響しないこと
#[test]
fn test_noop_pass_does_not_affect_execution() {
    let code = r#"
        let: x(10);
        let: y(20);
        func: main() {
            __trace(0);
            __trace(1);
            __trace(1);
            return: x + y;
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::syntactic_analyze(&s).unwrap();

    // 最適化なしの結果
    let trace_before = crate::interpret_func_testing(&scope, "main");
    let result_before = crate::interpret(&scope);

    // 最適化を適用
    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
        },
    );

    // 最適化ありの結果
    let trace_after = crate::interpret_func_testing(&scope, "main");
    let result_after = crate::interpret(&scope);

    assert_eq!(trace_before, trace_after, "trace results should be identical");
    assert_eq!(result_before, result_after, "return value should be identical");
}

/// 最適化なしの場合、Scope が変更されないこと
#[test]
fn test_no_optimization_leaves_scope_unchanged() {
    let code = "func: main() { return: 42; }".to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::syntactic_analyze(&s).unwrap();

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
        func: main() {
            __puti(42);
            return: 0;
        }
    "#
    .to_string();
    let t = crate::parse_to_tokens(&code).unwrap();
    let s = crate::parse_to_tree(&t).unwrap();
    let mut scope = crate::syntactic_analyze(&s).unwrap();

    optimizer::optimize(
        &mut scope,
        &OptimizationOptions {
            noop_test_pass: true,
        },
    );

    // Whitespace コンパイルが成功すること
    let result = crate::compiler_ws::compile_with_options(&scope, false, false);
    assert!(result.is_ok(), "WS compilation should succeed after optimization");
}
