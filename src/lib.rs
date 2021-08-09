use std::{iter, str};

pub enum Token {
    Number(i64),
    Symbol(char),
}

fn parse_number(iter: &mut iter::Peekable<str::Chars>) -> Token {
    let mut value = 0 as i64;
    while let Some(c) = iter.peek() {
        if !c.is_ascii_digit() {
            // TODO: minus
            // TODO: 0x
            break;
        }
        let d = c.to_digit(10).unwrap();
        value = value * 10 + d as i64;
        iter.next();
    }
    Token::Number(value)
}

pub fn parse_to_tokens(text: &String) -> Vec<Token> {
    let mut tokens = Vec::<Token>::new();
    let mut iter = text.chars().peekable();
    while let Some(c) = iter.peek() {
        if c.is_ascii_digit() {
            tokens.push(parse_number(&mut iter));
        } else if *c == '-' {
            tokens.push(Token::Symbol('-'));
            iter.next();
        } else if *c == '*' {
            tokens.push(Token::Symbol('*'));
            iter.next();
        } else if c.is_whitespace() {
            iter.next();
        } else {
            panic!("invalid char");
        }
    }
    tokens
}

//

pub enum Operator2 {
    Minus,
    Mul,
}

pub enum Expression {
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    Factor(i64),
}

fn parse_to_tree_factor(iter: &mut iter::Peekable<std::slice::Iter<Token>>) -> Box<Expression> {
    if let Some(token) = iter.peek() {
        match token {
            Token::Symbol(_) => panic!("syntax error: symbol"),
            Token::Number(val) => {
                iter.next();
                return Box::new(Expression::Factor(*val));
            }
        }
    }
    panic!("syntax error: terminal");
}

fn parse_to_tree_mul(iter: &mut iter::Peekable<std::slice::Iter<Token>>) -> Box<Expression> {
    let mut left = parse_to_tree_factor(iter);
    loop {
        let op = if let Some(token) = iter.peek() {
            match token {
                Token::Symbol(chr) => match *chr {
                    '*' => Operator2::Mul,
                    _ => return left,
                },
                _ => return left,
            }
        } else {
            return left;
        };
        iter.next();
        let right = parse_to_tree_factor(iter);
        left = Box::new(Expression::Operation2(op, left, right))
    }
}

fn parse_to_tree_plus(iter: &mut iter::Peekable<std::slice::Iter<Token>>) -> Box<Expression> {
    let mut left = parse_to_tree_mul(iter);
    loop {
        let op = if let Some(token) = iter.peek() {
            match token {
                Token::Symbol(chr) => match *chr {
                    '-' => Operator2::Minus,
                    _ => return left,
                },
                _ => return left,
            }
        } else {
            return left;
        };
        iter.next();
        let right = parse_to_tree_mul(iter);
        left = Box::new(Expression::Operation2(op, left, right));
    }
}

fn parse_to_tree_root(iter: &mut iter::Peekable<std::slice::Iter<Token>>) -> Box<Expression> {
    parse_to_tree_plus(iter)
}

pub fn parse_to_tree(tokens: &Vec<Token>) -> Box<Expression> {
    let mut iter = tokens.iter().peekable();
    parse_to_tree_root(&mut iter)
}

//

pub fn interpret_expression(expr: &Box<Expression>) -> i64 {
    match expr.as_ref() {
        Expression::Operation2(op, left, right) => match op {
            Operator2::Minus => interpret_expression(left) - interpret_expression(right),
            Operator2::Mul => interpret_expression(left) * interpret_expression(right),
        },
        Expression::Factor(v) => *v,
    }
}
