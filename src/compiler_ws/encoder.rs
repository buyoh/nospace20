//! Whitespace エンコーダ

use crate::compiler_ws::types::WsChar;

/// Vec<WsChar> を文字列に変換
#[allow(dead_code)]
pub fn to_string(chars: &[WsChar]) -> String {
    chars.iter().map(|c| c.to_char()).collect()
}
