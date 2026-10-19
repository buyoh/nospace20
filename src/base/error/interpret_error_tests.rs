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

#[test]
fn test_interpret_error_runtime_messages() {
    // runtime_error large テストの contains（従来の panic メッセージ）と一致する部分文字列を保つ
    let cases = [
        (InterpretError::DivisionByZero, "division by zero"),
        (InterpretError::AssertionFailed(None), "assertion failed"),
        (
            InterpretError::AssertionFailed(Some(3)),
            "assertion failed: 3 != 0",
        ),
        (
            InterpretError::InvalidMemoryAccess(5),
            "invalid memory access at address 5",
        ),
        (
            InterpretError::FreedMemoryAccess(5),
            "access to freed memory at address 5",
        ),
        (InterpretError::InvalidFree(7), "free: invalid address 7"),
        (
            InterpretError::FreeNotBlockStart {
                addr: 8,
                block_start: 7,
            },
            "free: address 8 is not a block start (block starts at 7)",
        ),
        (InterpretError::DoubleFree(7), "double free at address 7"),
        (
            InterpretError::InvalidAllocSize(-1),
            "alloc: invalid size -1",
        ),
        (
            InterpretError::InvalidCharCode(-1),
            "__putc: invalid character code -1",
        ),
        (
            InterpretError::ExpressionLimitExceeded(10),
            "expression evaluation limit exceeded: 10",
        ),
        (
            InterpretError::StackOverflow(100),
            "stack overflow: frame limit 100 exceeded",
        ),
        (
            InterpretError::Io("broken pipe".into()),
            "I/O error: broken pipe",
        ),
    ];
    for (err, msg) in cases {
        assert_eq!(err.to_string(), msg);
    }
}
