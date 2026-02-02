use super::*;

#[test]
fn test_char_index_to_line_single_line() {
    let code = TextCode::new("abc");
    assert_eq!(code.char_index_to_line(0), (0, 0));
    assert_eq!(code.char_index_to_line(1), (0, 1));
    assert_eq!(code.char_index_to_line(2), (0, 2));
}

#[test]
fn test_char_index_to_line_multi_line() {
    let code = TextCode::new("abc\ndef\nghi");
    assert_eq!(code.char_index_to_line(0), (0, 0));
    assert_eq!(code.char_index_to_line(3), (0, 3));
    assert_eq!(code.char_index_to_line(4), (1, 0));
    assert_eq!(code.char_index_to_line(7), (1, 3));
    assert_eq!(code.char_index_to_line(8), (2, 0));
}

#[test]
fn test_line_method() {
    let code = TextCode::new("abc\ndef");
    assert_eq!(code.line(0), "abc");
    assert_eq!(code.line(1), "def");
}

#[test]
fn test_line_with_carriage_return() {
    let code = TextCode::new("abc\r\ndef\r\n");
    assert_eq!(code.line(0), "abc");
    assert_eq!(code.line(1), "def");
}
