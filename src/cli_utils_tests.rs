use super::*;
use crate::optimizer::PASS_NAMES;

#[test]
fn test_cli_opt_pass_names_match_pass_names() {
    let mut cli: Vec<String> = CliOptPass::value_variants()
        .iter()
        .map(|p| p.pass_name())
        .collect();
    let mut expected: Vec<String> = PASS_NAMES.iter().map(|s| s.to_string()).collect();
    cli.sort();
    expected.sort();
    assert_eq!(cli, expected);
}

fn args_with_opt(opt: Vec<CliOptPass>) -> CliCompileArgs {
    CliCompileArgs {
        std: CliStd::Standard,
        std_ext: vec![],
        no_std_ext: false,
        opt,
    }
}

#[test]
fn test_build_optimization_options_peephole_only() {
    let o = args_with_opt(vec![CliOptPass::Peephole]).build_optimization_options();
    assert!(o.peephole);
    assert!(!o.any_enabled());
}

#[test]
fn test_build_optimization_options_all() {
    let o = args_with_opt(vec![CliOptPass::All]).build_optimization_options();
    assert!(o.peephole && o.condition_opt && o.dead_code);
}

#[test]
fn test_build_optimization_options_empty() {
    let o = args_with_opt(vec![]).build_optimization_options();
    assert!(!o.any_enabled() && !o.peephole);
}
