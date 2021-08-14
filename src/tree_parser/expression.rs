use std::iter;

use crate::token_parser::Token;

#[derive(Clone)] // TODO: REMOVE
pub enum Operator2 {
    Plus,
    Minus,
    Multiply,
    Divide,
    Assign,
}

#[derive(Clone)] // TODO: REMOVE
pub enum Expression {
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
}

fn parse_to_expression_tree_function(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
    name: &String,
) -> Box<Expression> {
    if let Some(Token::ParenthesisL) = iter.next() {
    } else {
        panic!("syntax error: symbol");
    }
    let mut args = Vec::<Box<Expression>>::new();
    enum State {
        L,
        Eval,
        Comma,
    }
    let mut state = State::L;
    loop {
        match iter.peek() {
            Some(Token::ParenthesisR) => {
                if let State::Comma = state {
                    panic!("syntax error: unexpected comma");
                }
                iter.next();
                return Box::new(Expression::Function(name.clone(), args));
            }
            Some(Token::Comma) => {
                if let State::Eval = state {
                    state = State::Comma;
                } else {
                    panic!("syntax error: unexpected comma");
                }
                iter.next();
            }
            _ => {
                if let State::Eval = state {
                    panic!("syntax error: missing comma");
                }
                let e = parse_to_expression_tree_root(iter);
                args.push(e);
                state = State::Eval;
            }
        }
    }
}

fn parse_to_expression_tree_factor(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    match iter.peek() {
        Some(Token::Number(val)) => {
            iter.next();
            return Box::new(Expression::Factor(*val));
        }
        Some(Token::Identifier(id)) => {
            // TODO: confirm whether the identifier is reserved e.g. func
            iter.next();
            if let Some(Token::ParenthesisL) = iter.peek() {
                return parse_to_expression_tree_function(iter, id);
            }
            return Box::new(Expression::Variable(id.clone()));
        }
        Some(Token::ParenthesisL) => {
            iter.next();
            let e = parse_to_expression_tree_root(iter);
            if let Some(Token::ParenthesisR) = iter.next() {
                return e;
            }
            panic!("syntax error: expected ')'");
        }
        _ => panic!("syntax error: terminal"),
    }
}

fn parse_to_expression_tree_mul(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    let mut left = parse_to_expression_tree_factor(iter);
    loop {
        let op = if let Some(token) = iter.peek() {
            match token {
                Token::Symbol(chr) => match *chr {
                    '*' => Operator2::Multiply,
                    '/' => Operator2::Divide,
                    _ => return left,
                },
                _ => return left,
            }
        } else {
            return left;
        };
        iter.next();
        let right = parse_to_expression_tree_factor(iter);
        left = Box::new(Expression::Operation2(op, left, right))
    }
}

fn parse_to_expression_tree_plus(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    let mut left = parse_to_expression_tree_mul(iter);
    loop {
        let op = if let Some(token) = iter.peek() {
            match token {
                Token::Symbol(chr) => match *chr {
                    '+' => Operator2::Plus,
                    '-' => Operator2::Minus,
                    _ => return left,
                },
                _ => return left,
            }
        } else {
            return left;
        };
        iter.next();
        let right = parse_to_expression_tree_mul(iter);
        left = Box::new(Expression::Operation2(op, left, right));
    }
}

fn parse_to_expression_tree_assign(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    let left = parse_to_expression_tree_plus(iter);
    let op = if let Some(token) = iter.peek() {
        match token {
            Token::Symbol(chr) => match *chr {
                '=' => Operator2::Assign,
                _ => return left,
            },
            _ => return left,
        }
    } else {
        return left;
    };
    iter.next();
    let right = parse_to_expression_tree_assign(iter);
    Box::new(Expression::Operation2(op, left, right))
}

pub(super) fn parse_to_expression_tree_root(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    parse_to_expression_tree_assign(iter)
}
