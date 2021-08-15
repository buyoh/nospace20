use syntactic_analyzer::Scope;
use token_parser::PrettyToken;
use tree_parser::Statement;

mod base;
mod interpreter;
mod syntactic_analyzer;
mod token_parser;
mod tree_parser;

pub fn parse_to_tokens(text: &String) -> Vec<PrettyToken> {
    token_parser::parse_to_tokens(text)
}

pub fn parse_to_tree(tokens: &Vec<PrettyToken>) -> Vec<Statement> {
    tree_parser::parse_to_tree(tokens)
}

pub fn syntactic_analyze(root: &Vec<Statement>) -> Scope {
    syntactic_analyzer::syntactic_analyze(root)
}

pub fn interpret_main_func(scope: &Scope) {
    interpreter::interpret_main_func(scope)
}
