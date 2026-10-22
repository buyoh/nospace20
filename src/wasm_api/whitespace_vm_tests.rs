use super::*;

/// 省略した引数は、3 つの構築関数で共通の既定値になる（メモリ上限は DEFAULT_MAX_MEMORY_CELLS_WASM）
#[test]
fn vm_options_defaults() {
    let options = vm_options_from_args(false, None, None, None);
    assert_eq!(
        options.max_memory_cells,
        Some(DEFAULT_MAX_MEMORY_CELLS_WASM)
    );
    assert!(!options.debug_ext);
    assert!(!options.strict_heap);
    assert!(!options.randomize_heap);
}
