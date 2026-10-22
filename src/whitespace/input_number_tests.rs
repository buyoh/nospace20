use super::parse_input_number;

// 期待値は wsc（tools/wsc-install/bin/wsc）で実測した結果（i64 の範囲外を除く）

#[test]
fn plain_and_signed_numbers() {
    assert_eq!(parse_input_number("1\n"), Some(1));
    assert_eq!(parse_input_number("+5\n"), Some(5));
    assert_eq!(parse_input_number("-5\n"), Some(-5));
    assert_eq!(parse_input_number("-0\n"), Some(0));
    assert_eq!(parse_input_number("007\n"), Some(7));
    // 末尾に改行がない最終行
    assert_eq!(parse_input_number("7"), Some(7));
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    assert_eq!(parse_input_number("  5  \n"), Some(5));
    assert_eq!(parse_input_number("1\t\n"), Some(1));
    assert_eq!(parse_input_number("1\r\n"), Some(1));
    assert_eq!(parse_input_number("\u{0b}5\n"), Some(5));
    assert_eq!(parse_input_number("\u{0c}5\n"), Some(5));
    // NBSP（Unicode の空白）
    assert_eq!(parse_input_number("\u{a0}5\n"), Some(5));
}

#[test]
fn underscores_after_first_digit_are_ignored() {
    assert_eq!(parse_input_number("1_000\n"), Some(1000));
    assert_eq!(parse_input_number("1__0\n"), Some(10));
    assert_eq!(parse_input_number("1_\n"), Some(1));
    assert_eq!(parse_input_number("-1_0\n"), Some(-10));
}

#[test]
fn underscore_before_first_digit_is_rejected() {
    assert_eq!(parse_input_number("_1\n"), None);
    assert_eq!(parse_input_number("-_1\n"), None);
    assert_eq!(parse_input_number("+_1\n"), None);
}

#[test]
fn malformed_lines_are_rejected() {
    for line in [
        "",
        "\n",
        "   \n",
        "1 2\n",
        "5x\n",
        "- 5\n",
        "--5\n",
        "+-5\n",
        "-\n",
        "+\n",
        "1e3\n",
        "0x10\n",
        "0b11\n",
        "5\r6\n",
        "1 _000\n",
        // 全角数字
        "\u{ff15}\n",
    ] {
        assert_eq!(parse_input_number(line), None, "{:?}", line);
    }
}

#[test]
fn i64_bounds() {
    assert_eq!(parse_input_number("-9223372036854775808\n"), Some(i64::MIN));
    assert_eq!(parse_input_number("9223372036854775807\n"), Some(i64::MAX));
    // wsc は多倍長で受け付けるが、本 VM は i64 のため範囲外はエラー
    assert_eq!(parse_input_number("9223372036854775808\n"), None);
    assert_eq!(parse_input_number("-9223372036854775809\n"), None);
    assert_eq!(parse_input_number("99999999999999999999\n"), None);
}
