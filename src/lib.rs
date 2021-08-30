#[cfg(test)]
#[macro_use]
extern crate assert_matches;

use std::collections::BTreeMap;

pub use base::CodeParseError;
use interpreter::Environment;
pub use logger::TextCode;
use syntactic_analyzer::Scope;
use token_parser::PrettyToken;
use tree_parser::Statement;

mod base;
mod interpreter;
mod logger;
mod syntactic_analyzer;
mod token_parser;
mod tree_parser;

pub fn parse_to_tokens(text: &String) -> Result<Vec<PrettyToken>, Vec<CodeParseError>> {
    match token_parser::parse_to_tokens(text) {
        Ok(x) => Ok(x),
        Err(err) => Err(err.iter().map(|e| e.shrink()).collect()),
    }
}

pub fn parse_to_tree(tokens: &Vec<PrettyToken>) -> Result<Vec<Statement>, Vec<CodeParseError>> {
    match tree_parser::parse_to_tree(tokens) {
        Ok(x) => Ok(x),
        Err(err) => Err(err.iter().map(|e| e.shrink()).collect()),
    }
}

pub fn syntactic_analyze(root: &Vec<Statement>) -> Scope {
    syntactic_analyzer::syntactic_analyze(root)
}

pub fn interpret_func(scope: &Scope, func_name: &str) -> Option<i64> {
    let mut env = Environment::new();
    interpreter::interpret_func(&mut env, scope, func_name)
}

pub fn interpret_func_testing(scope: &Scope, func_name: &str) -> BTreeMap<i64, i64> {
    let mut env = Environment::new();
    interpreter::interpret_func(&mut env, scope, func_name);
    env.traced
}
