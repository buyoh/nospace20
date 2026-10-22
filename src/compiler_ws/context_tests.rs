use super::*;
use crate::compiler_ws::alloc_runtime::BumpAllocRuntime;
use crate::semantic_analyzer::analyze;
use crate::{parse_to_tokens, parse_to_tree};

fn parse_and_analyze(code: &str) -> Program {
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
    ctx.with_pending_values(2, |ctx| {
        let local = ctx.enter_function(1, 0);
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
