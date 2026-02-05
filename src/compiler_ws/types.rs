//! 基本型定義

/// Whitespace の基本文字
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsChar {
    Space,  // SP (0x20)
    Tab,    // TB (0x09)
    Lf,     // LF (0x0A)
}

impl WsChar {
    pub fn to_char(&self) -> char {
        match self {
            WsChar::Space => ' ',
            WsChar::Tab => '\t',
            WsChar::Lf => '\n',
        }
    }
}

/// 数値パラメータ (符号付き整数)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsNumber(pub i64);

impl WsNumber {
    /// Whitespace 形式にエンコード
    pub fn encode(&self) -> Vec<WsChar> {
        use WsChar::*;
        
        let mut result = Vec::new();
        
        // 符号
        if self.0 < 0 {
            result.push(Tab);
        } else {
            result.push(Space);
        }
        
        // 絶対値をビット列に (MSB first)
        let abs_val = self.0.unsigned_abs();
        if abs_val == 0 {
            // 0 はビットなし
        } else {
            let bits = 64 - abs_val.leading_zeros();
            for i in (0..bits).rev() {
                if (abs_val >> i) & 1 == 1 {
                    result.push(Tab);
                } else {
                    result.push(Space);
                }
            }
        }
        
        // 終端
        result.push(Lf);
        result
    }
}

/// ラベル識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelId(pub u32);

impl LabelId {
    /// Whitespace 出力用の値
    pub fn to_ws_value(&self) -> i64 {
        self.0 as i64
    }
    
    /// オフセットを加算
    pub fn offset(&self, n: u32) -> LabelId {
        LabelId(self.0 + n)
    }
}

/// ヒープアドレス
/// 
/// Whitespace のヒープ上のアドレスを表す。
/// Debug 出力で実際の数値も確認可能。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeapAddress(pub i64);

impl HeapAddress {
    pub const fn new(addr: i64) -> Self {
        Self(addr)
    }
    
    /// アドレス値を取得（Whitespace 命令生成用）
    pub fn value(&self) -> i64 {
        self.0
    }
    
    /// オフセットを加算した新しいアドレスを返す
    pub fn offset(&self, n: i64) -> Self {
        Self(self.0 + n)
    }
}

impl std::fmt::Debug for HeapAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HeapAddr({})", self.0)
    }
}

impl std::fmt::Display for HeapAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_number_positive() {
        let n = WsNumber(5);
        // 5 = 101 (binary)
        // [Space (positive), Tab, Space, Tab, Lf]
        assert_eq!(
            n.encode(),
            vec![WsChar::Space, WsChar::Tab, WsChar::Space, WsChar::Tab, WsChar::Lf]
        );
    }
    
    #[test]
    fn test_encode_number_zero() {
        let n = WsNumber(0);
        assert_eq!(n.encode(), vec![WsChar::Space, WsChar::Lf]);
    }

    #[test]
    fn test_encode_number_negative() {
        let n = WsNumber(-1);
        // -1 = 1 (binary, abs)
        // [Tab (negative), Tab, Lf]
        assert_eq!(n.encode(), vec![WsChar::Tab, WsChar::Tab, WsChar::Lf]);
    }

    #[test]
    fn test_label_offset() {
        let l1 = LabelId(16);
        let l2 = l1.offset(5);
        assert_eq!(l2.0, 21);
    }

    #[test]
    fn test_heap_address_offset() {
        let addr = HeapAddress::new(100);
        let addr2 = addr.offset(50);
        assert_eq!(addr2.value(), 150);
    }
}
