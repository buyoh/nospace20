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

fn run_property(target_extensions: Vec<TargetExtension>, ignore_debug: bool) -> CompileProperty {
    CompileProperty {
        target_extensions,
        ignore_debug,
        ..property(ExecutionMode::Run)
    }
}

#[test]
fn run_compile_options_enables_debug_by_default() {
    // CLI 既定（--std-ext 指定なし、alloc は既定で有効）
    let opts = run_property(vec![TargetExtension::Alloc], false)
        .run_compile_options(OptimizationOptions::none());
    assert!(opts.debug_ext);
    assert!(opts.alloc_ext);
    assert_eq!(opts.output_format, WsOutputFormat::Whitespace);
}

#[test]
fn run_compile_options_ignore_debug_disables_debug() {
    let opts = run_property(vec![TargetExtension::Alloc], true)
        .run_compile_options(OptimizationOptions::none());
    assert!(!opts.debug_ext);
    assert!(opts.alloc_ext);
}

#[test]
fn run_compile_options_ignore_debug_wins_over_std_ext_debug() {
    // --ignore-debug と --std-ext debug を同時に指定した場合は --ignore-debug を優先する
    let opts = run_property(vec![TargetExtension::Debug, TargetExtension::Alloc], true)
        .run_compile_options(OptimizationOptions::none());
    assert!(!opts.debug_ext);
}

#[test]
fn run_compile_options_std_ext_debug_keeps_debug() {
    let opts = run_property(vec![TargetExtension::Debug, TargetExtension::Alloc], false)
        .run_compile_options(OptimizationOptions::none());
    assert!(opts.debug_ext);
}

#[test]
fn run_compile_options_no_std_ext_disables_alloc() {
    // --no-std-ext: target_extensions が空
    let opts =
        run_property(vec![], false).run_compile_options(OptimizationOptions::none());
    assert!(opts.debug_ext);
    assert!(!opts.alloc_ext);
}

#[test]
fn run_compile_options_passes_optimization() {
    let opts = run_property(vec![], false).run_compile_options(OptimizationOptions::all());
    assert!(opts.optimization.peephole);
    assert!(opts.optimization.any_enabled());
}
