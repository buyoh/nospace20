use crate::token_parser::Token;

pub(crate) use self::expression::Expression;
pub(crate) use self::expression::Operator2;
use self::statement::parse_to_statements;
pub(crate) use self::statement::Statement;

mod expression;
mod statement;

pub fn parse_to_tree(tokens: &Vec<Token>) -> Vec<Statement> {
    let mut iter = tokens.iter().peekable();
    parse_to_statements(&mut iter)
}
