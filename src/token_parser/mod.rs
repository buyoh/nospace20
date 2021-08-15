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

pub struct TokenInfo {
    pub code_pointer: usize,
}

pub type PrettyToken = (Token, TokenInfo);

impl TokenInfo {
    fn new(code_pointer: usize) -> Self {
        TokenInfo { code_pointer }
    }
}

fn parse_number(iter: &mut iter::Peekable<iter::Enumerate<str::Chars>>) -> Token {
    let mut value = 0 as i64;
    while let Some((_, c)) = iter.peek() {
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

fn parse_identifier(iter: &mut iter::Peekable<iter::Enumerate<str::Chars>>) -> Token {
    if let Some((_, 'A'..='Z')) | Some((_, 'a'..='z')) | Some((_, '_')) = iter.peek() {
    } else {
        panic!("internal error");
    }
    let mut id = String::new();
    loop {
        if let Some((_, 'A'..='Z')) | Some((_, 'a'..='z')) | Some((_, '_')) | Some((_, '0'..='9')) =
            iter.peek()
        {
            id.push(iter.next().unwrap().1);
        } else {
            id.shrink_to_fit();
            return Token::Identifier(id);
        }
    }
}

pub fn parse_to_tokens(text: &String) -> Vec<PrettyToken> {
    let mut tokens = Vec::<PrettyToken>::new();
    let mut iter = text.chars().enumerate().peekable();
    while let Some((idx, c)) = iter.peek() {
        let info = TokenInfo::new(*idx);
        if c.is_ascii_digit() {
            tokens.push((parse_number(&mut iter), info));
        } else if c.is_whitespace() {
            iter.next();
        } else {
            let t = match *c {
                '+' | '-' | '*' | '/' | '=' => Token::Symbol(*c),
                'A'..='Z' | 'a'..='z' | '_' => {
                    tokens.push((parse_identifier(&mut iter), info));
                    continue;
                }
                '(' => Token::ParenthesisL,
                ')' => Token::ParenthesisR,
                '[' => Token::BracketL,
                ']' => Token::BracketR,
                '{' => Token::BraceL,
                '}' => Token::BraceR,
                ';' => Token::Semicolon,
                ':' => Token::Colon,
                ',' => Token::Comma,
                _ => panic!("invalid char: {}", c),
            };
            tokens.push((t, info));
            iter.next();
        }
    }
    tokens
}
