//! WhitespaceVM のメモリ上限の既定値と、利用側（CLI・WASM）の設定値の変換

/// メモリ上限の推奨値（セル数）
///
/// CLI `nospace20 --mode run` の実行で使う。
///
/// 1 セルはホスト上でおよそ 16〜48 バイト。ヒープ中心のプログラムで上限に達したとき、
/// ホストのメモリのピークは約 0.85GB（x86_64 の release ビルドで実測 0.84GB）。
pub const DEFAULT_MAX_MEMORY_CELLS: usize = 1 << 24;

/// WASM `WasmWhitespaceVM::new` のメモリ上限の既定値（セル数）
///
/// wasm32 では `DEFAULT_MAX_MEMORY_CELLS` で WASM のメモリが約 1.09GB まで伸び（Node.js で実測。
/// WASM のメモリは縮まない）、ブラウザのタブが落ちうるため小さくする。この値では約 274MB。
/// large テスト（resources/tests）はすべて 1 << 20 セル以内で動作する。
pub const DEFAULT_MAX_MEMORY_CELLS_WASM: usize = 1 << 22;

/// CLI・WASM の設定値（`0` は無制限）を `WhitespaceVM::with_max_memory_cells` の引数に変換する
///
/// VM の API では `Some(0)` は「上限 0 セル」を意味するため、`0` の特別扱いはこの層で行う。
pub fn max_memory_cells_from_setting(cells: usize) -> Option<usize> {
    if cells == 0 {
        None
    } else {
        Some(cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_setting_is_unlimited() {
        assert_eq!(max_memory_cells_from_setting(0), None);
    }

    #[test]
    fn test_nonzero_setting_is_limit() {
        assert_eq!(max_memory_cells_from_setting(1), Some(1));
        assert_eq!(
            max_memory_cells_from_setting(DEFAULT_MAX_MEMORY_CELLS),
            Some(DEFAULT_MAX_MEMORY_CELLS)
        );
    }
}
