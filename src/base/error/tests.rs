use super::*;

#[test]
fn test_nospace_error_display_parse() {
    let errors = vec![
        CodeParseError::new(Some(5), "unexpected token"),
        CodeParseError::new(None, "generic error"),
    ];
    let err = NospaceError::Parse(errors);
    let s = format!("{}", err);
    assert!(s.contains("unexpected token"));
    assert!(s.contains("generic error"));
}

#[test]
fn test_nospace_error_display_compile() {
    let err = NospaceError::Compile(CompileError::new(CompileErrorKind::MainNotFound));
    assert_eq!(format!("{}", err), "__main function not found");
}

#[test]
fn test_nospace_error_display_interpret() {
    let err = NospaceError::Interpret(InterpretError::FunctionNotFound("foo".to_string()));
    assert_eq!(format!("{}", err), "function 'foo' not found");
}

#[test]
fn test_nospace_error_display_ws_runtime() {
    let err = NospaceError::WsRuntime(WsRuntimeError::StackUnderflow);
    assert_eq!(format!("{}", err), "stack underflow");
}

#[test]
fn test_nospace_error_display_ws_parse() {
    let err = NospaceError::WsParse(WsParseError::InvalidImp { position: 3 });
    assert_eq!(format!("{}", err), "invalid IMP at position 3");
}

#[test]
fn test_nospace_error_is_std_error() {
    let err = NospaceError::WsRuntime(WsRuntimeError::DivisionByZero);
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_nospace_error_from_vec_code_parse_error() {
    let errors = vec![CodeParseError::new(None, "err")];
    let err: NospaceError = errors.into();
    assert!(matches!(err, NospaceError::Parse(_)));
}

#[test]
fn test_nospace_error_from_compile_error() {
    let e = CompileError::new(CompileErrorKind::MainNotFound);
    let err: NospaceError = e.into();
    assert!(matches!(err, NospaceError::Compile(_)));
}

#[test]
fn test_nospace_error_from_interpret_error() {
    let e = InterpretError::FunctionNotFound("x".to_string());
    let err: NospaceError = e.into();
    assert!(matches!(err, NospaceError::Interpret(_)));
}

#[test]
fn test_nospace_error_from_ws_parse_error() {
    let e = WsParseError::InvalidImp { position: 0 };
    let err: NospaceError = e.into();
    assert!(matches!(err, NospaceError::WsParse(_)));
}

#[test]
fn test_nospace_error_from_ws_runtime_error() {
    let e = WsRuntimeError::StackUnderflow;
    let err: NospaceError = e.into();
    assert!(matches!(err, NospaceError::WsRuntime(_)));
}
