use super::*;

#[test]
fn test_ws_runtime_error_display() {
    assert_eq!(
        format!("{}", WsRuntimeError::StackUnderflow),
        "stack underflow"
    );
    assert_eq!(
        format!("{}", WsRuntimeError::DivisionByZero),
        "division by zero"
    );
    assert_eq!(
        format!("{}", WsRuntimeError::UndefinedLabel(42)),
        "undefined label: 42"
    );
    assert_eq!(
        format!("{}", WsRuntimeError::UninitializedHeap(10)),
        "uninitialized heap at address 10"
    );
    assert_eq!(
        format!("{}", WsRuntimeError::CallStackUnderflow),
        "call stack underflow"
    );
    assert_eq!(
        format!("{}", WsRuntimeError::ProgramCounterOutOfBounds),
        "program counter out of bounds"
    );
    assert_eq!(
        format!("{}", WsRuntimeError::IoError("io fail".to_string())),
        "I/O error: io fail"
    );
    assert_eq!(
        format!("{}", WsRuntimeError::AssertionFailed(99)),
        "assertion failed: 99"
    );
}

#[test]
fn test_ws_runtime_error_is_std_error() {
    let err = WsRuntimeError::StackUnderflow;
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_ws_parse_error_display() {
    assert_eq!(
        format!("{}", WsParseError::InvalidImp { position: 5 }),
        "invalid IMP at position 5"
    );
    assert_eq!(
        format!(
            "{}",
            WsParseError::InvalidCommand {
                position: 3,
                imp: "SS".to_string()
            }
        ),
        "invalid command for IMP 'SS' at position 3"
    );
    assert_eq!(
        format!(
            "{}",
            WsParseError::UnexpectedEof {
                context: "number".to_string()
            }
        ),
        "unexpected end of file while parsing number"
    );
    assert_eq!(
        format!("{}", WsParseError::InvalidNumber { position: 7 }),
        "invalid number at position 7"
    );
    assert_eq!(
        format!("{}", WsParseError::InvalidLabel { position: 2 }),
        "invalid label at position 2"
    );
    assert_eq!(
        format!(
            "{}",
            WsParseError::DuplicateLabel {
                label_id: 1,
                first_position: 10,
                second_position: 20
            }
        ),
        "duplicate label 1 (first at 10, second at 20)"
    );
}

#[test]
fn test_ws_parse_error_is_std_error() {
    let err = WsParseError::InvalidImp { position: 0 };
    let _: &dyn std::error::Error = &err;
}
