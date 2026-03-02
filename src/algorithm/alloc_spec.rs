//! アロケータアルゴリズムの共通仕様
//!
//! WS コンパイラ (`compiler_ws::alloc_runtime`) と
//! インタプリタ (`interpreter::allocator`) の両方から参照される。
//! アルゴリズムのパラメータと分類ロジックを一元管理し、
//! 実装間の不整合を防ぐ。

/// FSBA サイズクラス数
pub const FSBA_CLASS_COUNT: usize = 5;

/// FSBA サイズクラスのブロックサイズ（ヘッダー含む合計サイズ）
///
/// 各サイズクラスは固定サイズのフリーリストを持つ。
/// ユーザーリクエストの合計サイズ（ヘッダー込み）がこれ以下なら、
/// 対応するクラスの FSBA で確保される。
pub const FSBA_BLOCK_SIZES: [i64; FSBA_CLASS_COUNT] = [2, 4, 8, 16, 32];

/// ブロックヘッダーサイズ
///
/// 各ブロックの先頭 1 セルにブロック合計サイズが格納される。
/// ユーザーがアクセスできるのはヘッダーの次のセル（ptr = block + 1）以降。
pub const HEADER_SIZE: i64 = 1;

/// 最小ブロックサイズ（ヘッダー含む合計）
///
/// フリーリストでは block[0]=size, block[1]=next_ptr を使うため、
/// 最小 2 セルが必要。
pub const MIN_BLOCK_SIZE: i64 = 2;

/// ブロック分割時の最小残余サイズ
///
/// General alloc でブロックを分割する際、残余がこの値未満なら分割せず
/// ブロック全体を使用する。
pub const SPLIT_MIN_REMAINDER: i64 = 2;

/// ユーザーリクエストサイズから必要な合計サイズ（ヘッダー含む）を計算する。
///
/// - ヘッダー分 (+1) を加算
/// - 最小ブロックサイズ (2) 未満にならないよう保証
///
/// WS コンパイラではこのロジックを WS 命令として出力する。
/// インタプリタではこの関数を直接呼び出す。
pub const fn total_from_user_size(user_size: i64) -> i64 {
    let total = user_size + HEADER_SIZE;
    if total < MIN_BLOCK_SIZE {
        MIN_BLOCK_SIZE
    } else {
        total
    }
}

/// 合計サイズが属する FSBA サイズクラスのインデックスを返す。
///
/// 合計サイズが最大クラス (`FSBA_BLOCK_SIZES[FSBA_CLASS_COUNT-1]`) を超える場合は `None`。
/// `None` の場合、呼び出し元は汎用アロケータ（First-Fit + バンプ）を使う。
pub fn fsba_class_for(total_size: i64) -> Option<usize> {
    FSBA_BLOCK_SIZES
        .iter()
        .position(|&block_size| total_size <= block_size)
}

/// ブロック分割が可能かを判定する。
///
/// General alloc で見つかったブロックの合計サイズが `block_total_size` で、
/// 要求された合計サイズが `requested_total_size` のとき、
/// 残余 (`block_total_size - requested_total_size`) が `SPLIT_MIN_REMAINDER` 以上なら
/// ブロックを分割できる。
pub const fn can_split(block_total_size: i64, requested_total_size: i64) -> bool {
    block_total_size - requested_total_size >= SPLIT_MIN_REMAINDER
}

#[cfg(test)]
#[path = "alloc_spec_tests.rs"]
mod tests;
