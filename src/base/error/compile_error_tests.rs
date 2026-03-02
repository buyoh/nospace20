use super::*;

#[test]
fn test_compile_error_display() {
    let err = CompileError::new(CompileErrorKind::MainNotFound);
    assert_eq!(format!("{}", err), "__main function not found");
}

#[test]
fn test_compile_error_invalid_op() {
    let err = CompileError::new(CompileErrorKind::InvalidOperation("bad op".to_string()));
    assert_eq!(format!("{}", err), "Invalid operation: bad op");
}

#[test]
fn test_compile_error_is_std_error() {
    let err = CompileError::new(CompileErrorKind::MainNotFound);
    let _: &dyn std::error::Error = &err;
}
