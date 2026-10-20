use super::*;

fn property(mode: ExecutionMode) -> CompileProperty {
    CompileProperty {
        mode,
        ..Default::default()
    }
}

#[test]
fn run_without_compile_options_is_ok() {
    assert!(property(ExecutionMode::Run).validate().is_ok());
}

#[test]
fn run_with_target_is_rejected() {
    let p = CompileProperty {
        target: Some(CompileTarget::Ws),
        ..property(ExecutionMode::Run)
    };
    assert!(matches!(
        p.validate(),
        Err(ValidationError::CompileOnlyOption("--target"))
    ));
}

#[test]
fn run_with_output_is_rejected() {
    let p = CompileProperty {
        output: Some("out.ws".to_string()),
        ..property(ExecutionMode::Run)
    };
    assert!(matches!(
        p.validate(),
        Err(ValidationError::CompileOnlyOption("--output"))
    ));
}

#[test]
fn compile_with_target_and_output_is_ok() {
    let p = CompileProperty {
        target: Some(CompileTarget::Mnemonic),
        output: Some("out.txt".to_string()),
        ..property(ExecutionMode::Compile)
    };
    assert!(p.validate().is_ok());
}

#[test]
fn compile_with_json_target_is_unimplemented() {
    let p = CompileProperty {
        target: Some(CompileTarget::Json),
        ..property(ExecutionMode::Compile)
    };
    assert!(matches!(
        p.validate(),
        Err(ValidationError::UnimplementedFeature(_))
    ));
}

#[test]
fn target_or_default_is_ws_when_unspecified() {
    assert_eq!(
        property(ExecutionMode::Compile).target_or_default(),
        CompileTarget::Ws
    );
}

#[test]
fn compile_only_option_message() {
    assert_eq!(
        ValidationError::CompileOnlyOption("--target").to_string(),
        "--target is only valid with --mode=compile"
    );
}
