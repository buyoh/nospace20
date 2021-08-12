use std::iter;

use crate::token_parser::Token;

use super::expression::*;

pub enum Statement {
    VariableDeclaration(String, Box<Expression>),
    Expression(Box<Expression>),
}

fn parse_to_statements_let(iter: &mut iter::Peekable<std::slice::Iter<Token>>) -> Statement {
    iter.next(); // let
    if let Some(Token::Colon) = iter.next() {
        if let Some(Token::Identifier(id)) = iter.next() {
            if let Some(Token::Semicolon) = iter.next() {
                return Statement::VariableDeclaration(id.clone(), Box::new(Expression::Factor(0)));
            } else {
                panic!("syntax error: expected ';'");
            }
        } else {
            panic!("syntax error: expected ':'");
        }
    } else {
        panic!("syntax error: expected ':'");
    }
}

pub(super) fn parse_to_statements(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Vec<Statement> {
    let mut statements = Vec::<Statement>::new();
    while let Some(token) = iter.peek() {
        if let Token::Identifier(identifier) = token {
            if identifier == "let" {
                statements.push(parse_to_statements_let(iter));
                continue;
            }
        }
        let expr = parse_to_expression_tree_root(iter);
        if let Some(Token::Semicolon) = iter.next() {
            statements.push(Statement::Expression(expr));
        } else {
            panic!("syntax error: expected ';'");
        }
    }
    return statements;
    // panic!("syntax error: terminal");
}
