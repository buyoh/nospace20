#[cfg(test)]
#[macro_use]
extern crate assert_matches;

pub use base::CodeParseError;
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

pub fn interpret_main_func(scope: &Scope) {
    interpreter::interpret_main_func(scope)
}
