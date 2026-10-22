use super::*;

#[test]
fn default_property_is_ok() {
    assert!(CompileProperty::default().validate().is_ok());
}

#[test]
fn target_and_output_are_ok() {
    let p = CompileProperty {
        target: Some(CompileTarget::Mnemonic),
        output: Some("out.txt".to_string()),
        ..Default::default()
    };
    assert!(p.validate().is_ok());
}

#[test]
fn json_target_is_unimplemented() {
    let p = CompileProperty {
        target: Some(CompileTarget::Json),
        ..Default::default()
    };
    assert!(matches!(
        p.validate(),
        Err(ValidationError::UnimplementedFeature(_))
    ));
}

#[test]
fn target_or_default_is_ws_when_unspecified() {
    assert_eq!(
        CompileProperty::default().target_or_default(),
        CompileTarget::Ws
    );
}
