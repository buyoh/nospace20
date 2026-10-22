//! Whitespace ヒープの予約アドレス

/// Whitespace ヒープの予約アドレス
///
/// アドレス 0 は無効アドレスとして言語仕様上予約されている。
/// アドレス 0, 1 は未使用。最小のユーザー可視アドレスは GLOBAL_PTR(8) 。
/// アドレス 5,6,7 はアロケータ内部用。
/// ALLOC_FREE_HEAD(5)=0 は「空リスト」を表すセンチネルとして利用される。
/// 負のアドレスは拡張 API（`--std-ext debug`）用。
pub mod heap_layout {
    /// ローカルヒープの開始位置を格納するアドレス
    pub const LOCAL_HEAP_BEGIN: i64 = 2;
    /// ローカルヒープの終了位置を格納するアドレス
    pub const LOCAL_HEAP_END: i64 = 3;
    /// 一時ポインタ（内部使用）
    pub const TEMP_PTR: i64 = 4;
    /// グローバル変数領域の開始アドレス
    pub const GLOBAL_PTR: i64 = 8;

    // 拡張 API 用負ヒープアドレス (--std-ext debug)
    pub const EXT_TRACE_ADDR: i64 = -10;
    pub const EXT_ASSERT_ADDR: i64 = -11;
    pub const EXT_ASSERT_NOT_ADDR: i64 = -12;
    pub const EXT_CLOG_ADDR: i64 = -13;

    // メモリアロケータ用アドレス (--std-ext alloc)
    pub const ALLOC_FREE_HEAD: i64 = 5;
    pub const ALLOC_HEAP_TOP: i64 = 6;
    pub const FSBA_TABLE_PTR: i64 = 7;

    // FSBA サイズクラス数
    pub const FSBA_CLASS_COUNT: i64 = crate::algorithm::alloc_spec::FSBA_CLASS_COUNT as i64;

    // 正の予約アドレスは互いに異なり、すべて 0 より大きく GLOBAL_PTR より小さい
    // （グローバル変数の領域と重ならない）ことをコンパイル時に検査する
    const RESERVED: [i64; 6] = [
        LOCAL_HEAP_BEGIN,
        LOCAL_HEAP_END,
        TEMP_PTR,
        ALLOC_FREE_HEAD,
        ALLOC_HEAP_TOP,
        FSBA_TABLE_PTR,
    ];
    const _: () = {
        let mut i = 0;
        while i < RESERVED.len() {
            assert!(0 < RESERVED[i] && RESERVED[i] < GLOBAL_PTR);
            let mut j = i + 1;
            while j < RESERVED.len() {
                assert!(RESERVED[i] != RESERVED[j]);
                j += 1;
            }
            i += 1;
        }
    };
}
