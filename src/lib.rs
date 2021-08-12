use token_parser::Token;
use tree_parser::Statement;

mod interpreter;
mod token_parser;
mod tree_parser;

pub fn parse_to_tokens(text: &String) -> Vec<Token> {
    token_parser::parse_to_tokens(text)
}

pub fn parse_to_tree(tokens: &Vec<Token>) -> Vec<Statement> {
    tree_parser::parse_to_tree(tokens)
}

pub fn interpret_statements(statements: &Vec<Statement>) {
    interpreter::interpret_statements(statements)
}
