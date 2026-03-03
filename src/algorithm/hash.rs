//! 決定論的ハッシュ関数
//!
//! 未初期化メモリのフィル値生成に使用する LCG (Linear Congruential Generator)
//! ベースの決定論的ハッシュ関数を提供する。
//! デバッグ再現性を重視し、同じ入力に対して常に同じ出力を返す。
//!
//! インタプリタ (`interpreter`, `whitespace`) の複数モジュールから参照される。

/// LCG 乗数（Knuth の定数）
const LCG_MULTIPLIER: u64 = 6364136223846793005;

/// LCG 加算定数（Knuth の定数）
const LCG_INCREMENT: u64 = 1442695040888963407;

/// オフセット混合用の追加乗数
const OFFSET_MULTIPLIER: u64 = 2891336453;

/// 単一値の決定論的ハッシュ
///
/// 入力値から決定論的な非自明値を生成する。
/// 0 ではない値を返しやすくすることで、初期値 0 への暗黙依存バグを検出しやすくする。
pub fn lcg_hash(value: u64) -> u64 {
    value
        .wrapping_mul(LCG_MULTIPLIER)
        .wrapping_add(LCG_INCREMENT)
}

/// アドレスとオフセットから決定論的ハッシュを生成する
///
/// アドレスとオフセットの両方を混合し、同じ (addr, offset) ペアに対して
/// 常に同じ値を返す。
pub fn lcg_hash_with_offset(addr: i64, offset: usize) -> i64 {
    let seed = (addr as u64).wrapping_mul(LCG_MULTIPLIER)
        ^ (offset as u64).wrapping_mul(OFFSET_MULTIPLIER);
    seed.wrapping_add(LCG_INCREMENT) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcg_hash_deterministic() {
        // 同じ入力に対して常に同じ出力を返す
        assert_eq!(lcg_hash(42), lcg_hash(42));
        assert_eq!(lcg_hash(0), lcg_hash(0));
    }

    #[test]
    fn lcg_hash_nonzero_for_nonzero_input() {
        // 非ゼロ入力に対してゼロ以外を返しやすい
        // (0 を返す入力も理論上存在するが、ほとんどの入力で非ゼロ)
        let nonzero_count = (1..100u64).filter(|&v| lcg_hash(v) != 0).count();
        assert!(nonzero_count > 90);
    }

    #[test]
    fn lcg_hash_with_offset_deterministic() {
        assert_eq!(lcg_hash_with_offset(100, 5), lcg_hash_with_offset(100, 5));
    }

    #[test]
    fn lcg_hash_with_offset_varies_by_offset() {
        // 異なるオフセットで異なる値を返す
        let v0 = lcg_hash_with_offset(100, 0);
        let v1 = lcg_hash_with_offset(100, 1);
        assert_ne!(v0, v1);
    }
}
