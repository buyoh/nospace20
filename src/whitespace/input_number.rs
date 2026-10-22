//! InputNumber 命令（`__geti`）の 1 行の解釈
//!
//! wsc（whitespacers）の `readnum` と同じ規則で解釈する。
//! 仕様は `docs-ai/architecture/whitespace-runtime.md` の「InputNumber の入力の解釈」を参照。

/// 1 行を整数として解釈する
///
/// `line` は stdin から読んだ 1 行（末尾の `\n` を含んでもよい）。
/// 前後の Unicode 空白を除き、`[+-]?` の後に 10 進数字で始まり 10 進数字と `_` だけが続く文字列を
/// 受け付ける（`_` は無視する）。空行・不正な文字列・i64 の範囲外は `None`
pub(super) fn parse_input_number(line: &str) -> Option<i64> {
    let trimmed = line.trim();
    let (negative, body) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    if !body.as_bytes().first()?.is_ascii_digit() {
        return None;
    }
    if !body.bytes().all(|b| b.is_ascii_digit() || b == b'_') {
        return None;
    }

    // 負数は負の方向に積み上げる（i64::MIN を範囲外にしないため）
    let mut value: i64 = 0;
    for b in body.bytes().filter(|b| *b != b'_') {
        let digit = i64::from(b - b'0');
        value = value.checked_mul(10)?;
        value = if negative {
            value.checked_sub(digit)?
        } else {
            value.checked_add(digit)?
        };
    }
    Some(value)
}

#[cfg(test)]
#[path = "input_number_tests.rs"]
mod tests;
