use super::*;
use crate::semantic_analyzer::analyze;
use crate::{parse_to_tokens, parse_to_tree};

fn parse_and_analyze(code: &str) -> Scope {
    let code_string = code.to_string();
    let tokens = parse_to_tokens(&code_string).expect("Failed to parse tokens");
    let tree = parse_to_tree(&tokens).expect("Failed to parse tree");
    analyze(&tree).expect("Failed to analyze")
}

fn func_index(scope: &Scope, name: &str) -> usize {
    scope
        .symbol_table
        .function_names
        .iter()
        .position(|n| n == name)
        .expect("function not found")
}

#[test]
fn test_root_block_region_follows_globals_and_statics() {
    // グローバル 2 個・static 1 個・ルートのブロック変数 3 個（{} に 1、初期化子のブロックに 2）
    let scope = parse_and_analyze(
        "let: g; let: h({ let: a; let: b; 1; });
         { let: c; c = 1; };
         func: f() { static: s; return: s; }
         func: __main() { return: f(); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    assert_eq!(layout.root_init_block_range(), 3..6);
    assert_eq!(layout.total_size(), 6);
    assert_eq!(
        layout.static_var_offset(func_index(&scope, "f"), 0),
        Some(2)
    );
}

#[test]
fn test_static_init_frames_do_not_overlap() {
    // f: 非 static 変数 a（スロット 0）+ ブロック変数 1 個
    // q: 非 static 変数なし + ブロック変数 2 個
    let scope = parse_and_analyze(
        "let: g;
         func: f() { let: a; static: s1({ let: t; t = 1; t; }); return: s1; }
         func: q() { static: s2({ let: u; let: v; u = 1; v = 2; u + v; }); return: s2; }
         func: __main() { return: f() + q(); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    // [0,1) g, [1,3) s1 s2, ルートのブロック無し
    assert_eq!(layout.root_init_block_range(), 3..3);
    let f = layout.static_init_frame(func_index(&scope, "f")).unwrap();
    let q = layout.static_init_frame(func_index(&scope, "q")).unwrap();
    assert_eq!(f.block_base - f.frame_base, 1);
    assert_eq!(f.end - f.block_base, 1);
    assert_eq!(q.block_base - q.frame_base, 0);
    assert_eq!(q.end - q.block_base, 2);
    assert!(f.end <= q.frame_base || q.end <= f.frame_base);
    assert!(f.frame_base >= 3 && q.frame_base >= 3);
    assert_eq!(layout.total_size(), 7);
    assert_eq!(layout.static_init_frame(func_index(&scope, "__main")), None);
}

#[test]
fn test_static_init_frame_excludes_static_slots() {
    // 非 static 変数 x はスロット 3（s: 0, buf: 1..3 の後）。static 配列の分も含めて x まで確保する
    // 一方、最後の非 static 変数より後ろの static 配列は確保しない
    let scope = parse_and_analyze(
        "func: f() { static: s(1); static: buf[2]; let: x; static: big[100]; x = 1; return: s; }
         func: __main() { return: f(); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    let frame = layout.static_init_frame(func_index(&scope, "f")).unwrap();
    assert_eq!(frame.block_base - frame.frame_base, 4);
    assert_eq!(frame.end, frame.block_base);
    // static 103 スロット + フレーム相当 4 スロット
    assert_eq!(layout.total_size(), 103 + 4);
}

#[test]
fn test_unused_function_static_init_is_reserved() {
    let scope = parse_and_analyze(
        "func: unused() { static: s({ let: t; t = 1; t; }); return: s; }
         func: __main() { return: 0; }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    let frame = layout
        .static_init_frame(func_index(&scope, "unused"))
        .unwrap();
    // static s の直後に、ブロック変数 1 個
    assert_eq!(
        frame,
        StaticInitFrame {
            frame_base: 1,
            block_base: 1,
            end: 2
        }
    );
    assert_eq!(layout.total_size(), 2);
}

#[test]
fn test_no_init_blocks_keeps_previous_size() {
    // 初期化文にブロックが無く、static 初期化子から参照しうる非 static 変数も無い
    // → グローバル変数 + static 変数のみ（従来と同じサイズ）
    let scope = parse_and_analyze(
        "let: g(1); let: arr[3];
         func: f() { static: s; static: t[2]; return: s; }
         func: h() { let: x; x = 1; return: x; }
         func: __main() { return: f() + h(); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    assert_eq!(layout.total_size(), 4 + 3);
    assert_eq!(layout.root_init_block_range(), 7..7);
}

#[test]
fn test_static_init_frame_reserves_non_static_vars_even_if_unreferenced() {
    // 初期化子の無い `static: s;` も static 初期化文を持つため、引数・非 static 変数の分を確保する
    let scope = parse_and_analyze(
        "func: f(a, b) { let: arr[3]; static: s; return: s + a; }
         func: __main() { return: f(1, 2); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    let frame = layout.static_init_frame(func_index(&scope, "f")).unwrap();
    // a, b, arr[3] の 5 スロット（static s はスロット 5）
    assert_eq!(frame.block_base - frame.frame_base, 5);
    assert_eq!(layout.total_size(), 1 + 5);
}

#[test]
fn test_huge_but_representable_array_compiles() {
    // 巨大でも WS のアドレス（i64）に収まるサイズは受理する（実際のメモリ使用量は実行時に制限される）
    let scope = parse_and_analyze(
        "func: __main() { let: a[9223372036854775000]; a[0] = 1; __puti(a[0]); }",
    );
    assert!(crate::compiler_ws::compile_with_options(&scope, false, false).is_ok());
    assert!(crate::compiler_ws::compile_with_options(&scope, false, true).is_ok());
}

/// GLOBAL_PTR + グローバル領域 + アロケータの管理領域が i64::MAX ちょうどになるグローバル配列のサイズ
fn max_global_array_size(tail_slots: i64) -> i64 {
    i64::MAX - crate::compiler_ws::memory::heap_layout::GLOBAL_PTR - tail_slots
}

#[test]
fn test_global_area_boundary_without_tail() {
    let size = max_global_array_size(0);
    let ok = parse_and_analyze(&format!("let: g[{}]; func: __main() {{}}", size));
    assert!(GlobalLayout::compute(&ok, 0).is_ok());
    let over = parse_and_analyze(&format!("let: g[{}]; func: __main() {{}}", size + 1));
    assert!(GlobalLayout::compute(&over, 0).is_err());
}

#[test]
fn test_global_area_boundary_with_allocator_tail() {
    let tail = crate::compiler_ws::memory::heap_layout::FSBA_CLASS_COUNT;
    let size = max_global_array_size(tail);
    let ok = parse_and_analyze(&format!("let: g[{}]; func: __main() {{}}", size));
    assert!(GlobalLayout::compute(&ok, tail).is_ok());
    let over = parse_and_analyze(&format!("let: g[{}]; func: __main() {{}}", size + 1));
    assert!(GlobalLayout::compute(&over, tail).is_err());
    // 同じサイズでも、管理領域を置かないアロケータなら収まる
    assert!(GlobalLayout::compute(&over, 0).is_ok());
}

#[test]
fn test_global_area_boundary_compiles_with_both_allocators() {
    // コード生成（FSBA の memory init のアドレス計算を含む）まで panic しないこと
    let tail = crate::compiler_ws::memory::heap_layout::FSBA_CLASS_COUNT;
    let fsba_max = parse_and_analyze(&format!(
        "let: g[{}]; func: __main() {{}}",
        max_global_array_size(tail)
    ));
    assert!(crate::compiler_ws::compile_with_options(&fsba_max, false, true).is_ok());
    let bump_max = parse_and_analyze(&format!(
        "let: g[{}]; func: __main() {{}}",
        max_global_array_size(0)
    ));
    assert!(crate::compiler_ws::compile_with_options(&bump_max, false, false).is_ok());
    assert!(crate::compiler_ws::compile_with_options(&bump_max, false, true).is_err());
}
