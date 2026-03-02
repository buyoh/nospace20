use super::*;

#[test]
fn test_interpret_error_function_not_found() {
    let err = InterpretError::FunctionNotFound("foo".to_string());
    assert_eq!(format!("{}", err), "function 'foo' not found");
}

#[test]
fn test_interpret_error_unexpected_flow() {
    let err = InterpretError::UnexpectedFlow("in static init".to_string());
    assert_eq!(format!("{}", err), "unexpected flow: in static init");
}

#[test]
fn test_interpret_error_is_std_error() {
    let err = InterpretError::FunctionNotFound("bar".to_string());
    let _: &dyn std::error::Error = &err;
}
