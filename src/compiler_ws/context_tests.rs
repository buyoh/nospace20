use super::*;
use crate::compiler_ws::alloc_runtime::BumpAllocRuntime;
use crate::semantic_analyzer::analyze;
use crate::{parse_to_tokens, parse_to_tree};

fn parse_and_analyze(code: &str) -> Scope {
    let code_string = code.to_string();
    let tokens = parse_to_tokens(&code_string).expect("Failed to parse tokens");
    let tree = parse_to_tree(&tokens).expect("Failed to parse tree");
    analyze(&tree).expect("Failed to analyze")
}

const CODE: &str = "func: __main() { return: 0; }";

#[test]
fn test_with_pending_values_restores_count_after_ok_and_err() {
    let scope = parse_and_analyze(CODE);
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    let mut ctx =
        CodeGenContext::new_with_options(&scope, &layout, false, false, &BumpAllocRuntime);

    let inner = ctx.with_pending_values(2, |ctx| {
        ctx.with_pending_values(1, |ctx| ctx.pending_stack_values())
    });
    assert_eq!(inner, 3);
    assert_eq!(ctx.pending_stack_values(), 0);

    // `?` で抜けた場合も個数が戻る
    let result: Result<(), ()> = ctx.with_pending_values(2, |_| Err(()));
    assert!(result.is_err());
    assert_eq!(ctx.pending_stack_values(), 0);
}

#[test]
fn test_enter_function_starts_with_zero_pending_values() {
    let scope = parse_and_analyze(CODE);
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    let mut ctx =
        CodeGenContext::new_with_options(&scope, &layout, false, false, &BumpAllocRuntime);
    let func = &scope.functions[0];

    ctx.with_pending_values(2, |ctx| {
        let local = ctx.enter_function(1, 0, 0, &func.block.scope);
        assert_eq!(local.pending_stack_values(), 0);
    });
}

#[test]
fn test_pending_values_in_loop_counts_from_loop_entry() {
    let scope = parse_and_analyze(CODE);
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    let mut ctx =
        CodeGenContext::new_with_options(&scope, &layout, false, false, &BumpAllocRuntime);
    assert_eq!(ctx.pending_stack_values_in_loop(), None);

    ctx.with_pending_values(2, |ctx| {
        let (start, end) = (ctx.new_label(), ctx.new_label());
        ctx.push_loop_labels(start, end);
        assert_eq!(ctx.pending_stack_values_in_loop(), Some(0));
        ctx.with_pending_values(3, |ctx| {
            assert_eq!(ctx.pending_stack_values_in_loop(), Some(3));
            assert_eq!(ctx.pending_stack_values(), 5);
        });
        ctx.pop_loop_labels();
    });
    assert_eq!(ctx.pending_stack_values_in_loop(), None);
}

#[test]
fn test_enter_function_for_static_init_starts_with_zero_pending_values() {
    let scope = parse_and_analyze(
        "func: f() { static: s({ 1; }); return: s; }
         func: __main() { return: f(); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    let mut ctx =
        CodeGenContext::new_with_options(&scope, &layout, false, false, &BumpAllocRuntime);
    let func_idx = scope
        .symbol_table
        .function_names
        .iter()
        .position(|n| n == "f")
        .expect("function not found");
    let func_scope = &scope.functions[func_idx].block.scope;

    ctx.with_pending_values(2, |ctx| {
        let static_ctx = ctx.enter_function_for_static_init(func_idx, func_scope);
        assert_eq!(static_ctx.pending_stack_values(), 0);
    });
}
