use super::*;
use crate::semantic_analyzer::analyze;
use crate::{parse_to_tokens, parse_to_tree};

fn parse_and_analyze(code: &str) -> Program {
    let code_string = code.to_string();
    let tokens = parse_to_tokens(&code_string).expect("Failed to parse tokens");
    let tree = parse_to_tree(&tokens).expect("Failed to parse tree");
    analyze(&tree).expect("Failed to analyze")
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
    // グローバル領域 = ルートの変数 2 個 + 関数の static 1 個
    assert_eq!(scope.global_slot_count, 3);
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    assert_eq!(layout.root_init_block_range(), 3..6);
    assert_eq!(layout.total_size(), 6);
}

#[test]
fn test_function_static_initializer_blocks_use_root_init_region() {
    // 関数の static の初期化子はルートの文脈で実行されるので、その中のブロック変数も
    // ルート初期化文の領域に数える（以前は関数ごとのフレーム相当領域を予約していた）
    let scope = parse_and_analyze(
        "let: g;
         func: f() { static: s1({ let: t; t = 1; t; }); return: s1; }
         func: q() { static: s2({ let: u; let: v; u = 1; v = 2; u + v; }); return: s2; }
         func: __main() { return: f() + q(); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    // [0,1) g, [1,3) s1 s2, [3,6) 初期化子のブロック変数 t / u v
    assert_eq!(layout.root_init_block_range(), 3..6);
    assert_eq!(layout.total_size(), 6);
}

#[test]
fn test_function_statics_are_in_global_region_not_in_frame() {
    // 関数スコープの static（配列を含む）はグローバル領域に置かれ、関数のフレームのスロットを消費しない
    let scope = parse_and_analyze(
        "func: f() { static: s(1); static: buf[2]; let: x; static: big[100]; x = 1; return: s; }
         func: __main() { return: f(); }",
    );
    assert_eq!(scope.global_slot_count, 103);
    let f = scope.get_function("f").expect("f must exist");
    assert_eq!(f.block.scope.variable_count, 1);
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    assert_eq!(layout.total_size(), 103);
}

#[test]
fn test_unused_function_static_init_is_reserved() {
    // 使われない関数の static も初期化するので、その初期化子のブロック変数も予約する
    let scope = parse_and_analyze(
        "func: unused() { static: s({ let: t; t = 1; t; }); return: s; }
         func: __main() { return: 0; }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    // static s の直後に、ブロック変数 1 個
    assert_eq!(layout.root_init_block_range(), 1..2);
    assert_eq!(layout.total_size(), 2);
}

#[test]
fn test_no_init_blocks_keeps_previous_size() {
    // 初期化文にブロックが無い → グローバル変数 + static 変数のみ
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
fn test_function_non_static_vars_are_not_reserved() {
    // static の初期化子は関数の非 static 変数を参照できないので、引数・非 static 変数の分は予約しない
    // （以前は StaticInitFrame がその分を確保していた）
    let scope = parse_and_analyze(
        "func: f(a, b) { let: arr[3]; static: s; return: s + a; }
         func: __main() { return: f(1, 2); }",
    );
    let layout = GlobalLayout::compute(&scope, 0).expect("layout must fit");
    assert_eq!(layout.total_size(), 1);
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
