use super::*;

/// 省略した引数は、3 つの構築関数で共通の既定値になる（メモリ上限は DEFAULT_MAX_MEMORY_CELLS_WASM）
#[test]
fn vm_options_defaults() {
    let options = vm_options_from_args(false, None, None, None).unwrap();
    assert_eq!(
        options.max_memory_cells,
        Some(DEFAULT_MAX_MEMORY_CELLS_WASM)
    );
    assert!(!options.debug_ext);
    assert!(!options.strict_heap);
    assert!(!options.randomize_heap);
}

/// 0 は無制限、u32 の範囲の非負整数はそのまま上限になる
#[test]
fn max_memory_cells_accepts_non_negative_integers() {
    assert_eq!(max_memory_cells_from_js(Some(0.0)), Ok(None));
    assert_eq!(max_memory_cells_from_js(Some(-0.0)), Ok(None));
    assert_eq!(max_memory_cells_from_js(Some(1000.0)), Ok(Some(1000)));
    assert_eq!(
        max_memory_cells_from_js(Some(u32::MAX as f64)),
        Ok(Some(u32::MAX as usize))
    );
}

/// 負数・小数・NaN・無限大・u32 を超える値はエラー（以前は u32 に回り込んで受け付けていた）
#[test]
fn max_memory_cells_rejects_invalid_values() {
    for value in [
        -1.0,
        1.5,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        u32::MAX as f64 + 1.0,
    ] {
        let err = max_memory_cells_from_js(Some(value)).expect_err("must be rejected");
        assert!(err.contains("invalid max_memory_cells"), "{}", err);
    }
    assert!(vm_options_from_args(false, Some(-1.0), None, None).is_err());
}
