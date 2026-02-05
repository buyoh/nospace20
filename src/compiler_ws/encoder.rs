//! Whitespace エンコーダ

use crate::compiler_ws::types::WsChar;

/// Whitespace コード文字列にエンコード
pub trait WsEncode {
    fn encode(&self) -> Vec<WsChar>;
}

/// Vec<WsChar> を文字列に変換
pub fn to_string(chars: &[WsChar]) -> String {
    chars.iter().map(|c| c.to_char()).collect()
}
