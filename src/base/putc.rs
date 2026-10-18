//! `__putc` / OutputChar の出力バイト列エンコード仕様
//!
//! - 0..=255: 生バイトをそのまま 1 バイト出力する（`__getc` のバイト単位読み込みと対称）
//! - 256 以上: Unicode スカラ値として UTF-8 エンコードして出力する
//! - 負値・サロゲート・Unicode 範囲外: 出力不可（`None`）

/// 出力すべきバイト列を返す。出力不可の値は `None`。
pub fn encode_putc(val: i64) -> Option<Vec<u8>> {
    if (0..=255).contains(&val) {
        return Some(vec![val as u8]);
    }
    let code = u32::try_from(val).ok()?;
    let ch = char::from_u32(code)?;
    let mut buf = [0u8; 4];
    Some(ch.encode_utf8(&mut buf).as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_byte_range() {
        assert_eq!(encode_putc(65), Some(vec![65]));
        assert_eq!(encode_putc(200), Some(vec![200]));
        assert_eq!(encode_putc(255), Some(vec![255]));
    }

    #[test]
    fn utf8_for_256_and_above() {
        assert_eq!(encode_putc(0x3042), Some(vec![0xe3, 0x81, 0x82]));
        assert_eq!(encode_putc(256), Some(vec![0xc4, 0x80]));
    }

    #[test]
    fn invalid_values() {
        assert_eq!(encode_putc(-1), None);
        assert_eq!(encode_putc(0xD800), None);
        assert_eq!(encode_putc(0x110000), None);
    }
}
