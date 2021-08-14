use std::{iter, str};

pub enum Token {
    Number(i64),
    Identifier(String),
    Symbol(char),
    ParenthesisL, // (
    ParenthesisR, // )
    BracketL,     // [
    BracketR,     // ]
    BraceL,       // {
    BraceR,       // }
    Semicolon,    // ;
    Colon,        // ;
    Comma,        //,
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

fn parse_identifier(iter: &mut iter::Peekable<str::Chars>) -> Token {
    if let Some('A'..='Z') | Some('a'..='z') | Some('_') = iter.peek() {
    } else {
        panic!("internal error");
    }
    let mut id = String::new();
    loop {
        if let Some('A'..='Z') | Some('a'..='z') | Some('_') | Some('0'..='9') = iter.peek() {
            id.push(iter.next().unwrap());
        } else {
            id.shrink_to_fit();
            return Token::Identifier(id);
        }
    }
}

pub fn parse_to_tokens(text: &String) -> Vec<Token> {
    let mut tokens = Vec::<Token>::new();
    let mut iter = text.chars().peekable();
    while let Some(c) = iter.peek() {
        if c.is_ascii_digit() {
            tokens.push(parse_number(&mut iter));
        } else if c.is_whitespace() {
            iter.next();
        } else {
            match *c {
                '+' | '-' | '*' | '/' | '=' => {
                    tokens.push(Token::Symbol(*c));
                    iter.next();
                }
                'A'..='Z' | 'a'..='z' | '_' => {
                    tokens.push(parse_identifier(&mut iter));
                }
                '(' => {
                    tokens.push(Token::ParenthesisL);
                    iter.next();
                }
                ')' => {
                    tokens.push(Token::ParenthesisR);
                    iter.next();
                }
                '[' => {
                    tokens.push(Token::BracketL);
                    iter.next();
                }
                ']' => {
                    tokens.push(Token::BracketR);
                    iter.next();
                }
                '{' => {
                    tokens.push(Token::BraceL);
                    iter.next();
                }
                '}' => {
                    tokens.push(Token::BraceR);
                    iter.next();
                }
                ';' => {
                    tokens.push(Token::Semicolon);
                    iter.next();
                }
                ':' => {
                    tokens.push(Token::Colon);
                    iter.next();
                }
                ',' => {
                    tokens.push(Token::Comma);
                    iter.next();
                }
                _ => panic!("invalid char: {}", c),
            }
        }
    }
    tokens
}
