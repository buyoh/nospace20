use super::*;

#[test]
fn target_or_default_is_ws_when_unspecified() {
    assert_eq!(
        CompileProperty::default().target_or_default(),
        CompileTarget::Ws
    );
}
