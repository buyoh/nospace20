use std::iter;

use crate::token_parser::{PrettyToken, Token};

use super::expression::*;

#[derive(Clone)] // TODO: REMOVE
pub enum Statement {
    VariableDeclaration(String, Box<Expression>),
    FunctionDeclaration(String, Vec<String>, Vec<Statement>),
    Return(Box<Expression>),
    Expression(Box<Expression>),
}

fn parse_to_statements_block(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> Vec<Statement> {
    if let Some((Token::BraceL, _)) = iter.next() {
    } else {
        panic!("syntax error: expected left brace");
    }
    let ss = parse_to_statements(iter);
    if let Some((Token::BraceR, _)) = iter.next() {
        return ss;
    } else {
        panic!("syntax error: expected right brace");
    }
}

fn parse_to_statements_let(iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>) -> Statement {
    iter.next(); // let
    if let Some((Token::Colon, _)) = iter.next() {
    } else {
        panic!("syntax error: expected ':'");
    }
    let id = if let Some((Token::Identifier(id), _)) = iter.next() {
        id
    } else {
        panic!("syntax error: expected ':'");
    };
    if let Some((Token::Semicolon, _)) = iter.next() {
        return Statement::VariableDeclaration(id.clone(), Box::new(Expression::Factor(0)));
    } else {
        panic!("syntax error: expected ';'");
    }
}

fn parse_to_statements_func(iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>) -> Statement {
    iter.next(); // func
    if let Some((Token::Colon, _)) = iter.next() {
    } else {
        panic!("syntax error: expected ':'");
    }
    let id = if let Some((Token::Identifier(id), _)) = iter.next() {
        id
    } else {
        panic!("syntax error: expected identifier");
    };
    if let Some((Token::ParenthesisL, _)) = iter.next() {
    } else {
        panic!("syntax error: expected '('");
    }
    let mut args = Vec::<String>::new();
    enum State {
        L,
        Var,
        Comma,
    }
    let mut state = State::L;
    loop {
        match iter.next() {
            Some((Token::Identifier(name), _)) => {
                if let State::Var = state {
                    panic!("syntax error: expected ','");
                }
                args.push(name.clone());
                state = State::Var;
            }
            Some((Token::Comma, _)) => {
                if let State::Var = state {
                    state = State::Comma;
                } else {
                    panic!("syntax error: unexpected ','");
                }
            }
            Some((Token::ParenthesisR, _)) => {
                if let State::Comma = state {
                    panic!("syntax error: unexpected ','");
                } else {
                    break;
                }
            }
            _ => {
                panic!("syntax error");
            }
        }
    }
    if let Some((Token::BraceL, _)) = iter.peek() {
        return Statement::FunctionDeclaration(id.clone(), args, parse_to_statements_block(iter));
    } else {
        panic!("syntax error: expected left brace");
    }
}

fn parse_to_statements_return(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> Statement {
    iter.next(); // return
    if let Some((Token::Colon, _)) = iter.next() {
    } else {
        panic!("syntax error: expected ':'");
    }
    let expr = parse_to_expression_tree_root(iter);
    if let Some((Token::Semicolon, _)) = iter.next() {
        return Statement::Return(expr);
    } else {
        panic!("syntax error: expected ';'");
    }
}

pub(super) fn parse_to_statements(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> Vec<Statement> {
    let mut statements = Vec::<Statement>::new();
    while let Some(token) = iter.peek() {
        match token {
            (Token::Identifier(identifier), _) => {
                if identifier == "let" {
                    statements.push(parse_to_statements_let(iter));
                    continue;
                }
                if identifier == "func" {
                    statements.push(parse_to_statements_func(iter));
                    continue;
                }
                if identifier == "return" {
                    statements.push(parse_to_statements_return(iter));
                    continue;
                }
            }
            (Token::BraceR, _) => {
                // TODO: consider only BraceR
                break;
            }
            _ => {}
        }
        let expr = parse_to_expression_tree_root(iter);
        if let Some((Token::Semicolon, _)) = iter.next() {
            statements.push(Statement::Expression(expr));
        } else {
            panic!("syntax error: expected ';'");
        }
    }
    return statements;
    // panic!("syntax error: terminal");
}
