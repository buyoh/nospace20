use super::*;

#[test]
fn test_code_parse_error_display_with_position() {
    let err = CodeParseError::new(Some(42), "unexpected token");
    assert_eq!(format!("{}", err), "at position 42: unexpected token");
}

#[test]
fn test_code_parse_error_display_without_position() {
    let err = CodeParseError::new(None, "generic error");
    assert_eq!(format!("{}", err), "generic error");
}

#[test]
fn test_code_parse_error_is_std_error() {
    let err = CodeParseError::new(Some(0), "test");
    let _: &dyn std::error::Error = &err;
}
