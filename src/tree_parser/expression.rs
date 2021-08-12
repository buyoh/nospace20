use std::iter;

use crate::token_parser::Token;

pub enum Operator2 {
    Plus,
    Minus,
    Multiply,
    Divide,
    Assign,
}

pub enum Expression {
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
}

fn parse_to_expression_tree_function(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    if let Some(Token::Identifier(name)) = iter.next() {
        if let Some(Token::ParenthesisL) = iter.next() {
            // TODO: unary only
            let e = parse_to_expression_tree_root(iter);
            if let Some(Token::ParenthesisR) = iter.next() {
                return Box::new(Expression::Function(name.clone(), vec![e]));
            }
        }
    }
    panic!("syntax error: symbol");
}

fn parse_to_expression_tree_factor(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    if let Some(token) = iter.peek() {
        match token {
            Token::Number(val) => {
                iter.next();
                return Box::new(Expression::Factor(*val));
            }
            Token::Identifier(id) => match id.as_str() {
                "one" => {
                    iter.next();
                    return Box::new(Expression::Factor(1));
                }
                "two" => {
                    iter.next();
                    return Box::new(Expression::Factor(2));
                }
                "pow2" => return parse_to_expression_tree_function(iter),
                _ => {
                    // TODO: confirm whether the identifier is declared
                    // panic!("syntax error: unknown identifier")
                    iter.next();
                    return Box::new(Expression::Variable(id.clone()));
                }
            },
            Token::ParenthesisL => {
                iter.next();
                let e = parse_to_expression_tree_root(iter);
                if let Some(Token::ParenthesisR) = iter.next() {
                    return e;
                }
                panic!("syntax error: expected ')'");
            }
            _ => panic!("syntax error: symbol"),
        }
    }
    panic!("syntax error: terminal");
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
