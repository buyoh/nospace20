//! # Token Parser
//!
//! このモジュールは、文字列をトークンと呼ばれる単位に分割し、トークン列を返します。
//! コメント文やスペース等の意味を成さない文字列は、このモジュールによって取り除かれます。

use std::{iter, str};

use crate::{base::CodeParseError, code_parse_error};

#[derive(Debug)]
pub enum Keyword {
    Let,
    Func,
    If,
    Else,
    While,
    Return,
    Break,
    Continue,
}

#[derive(Debug)]
pub enum Token {
    Number(i64),
    Identifier(String),
    Keyword(Keyword),
    Plus,
    Minus,
    Asterisk,
    Slash,
    Percent,       // %
    Exclamation,
    SingleEqual,
    DoubleEqual,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    DoubleAmpersand, // &&
    DoublePipe,      // ||
    ParenthesisL, // (
    ParenthesisR, // )
    BracketL,     // [
    BracketR,     // ]
    BraceL,       // {
    BraceR,       // }
    Semicolon,    // ;
    Colon,        // :
    Comma,        // ,
    Invalid,
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

fn parse_number(iter: &mut iter::Peekable<iter::Enumerate<impl Iterator<Item = char>>>) -> Token {
    // token レベルでは負の数を扱うことはできない
    let mut value = 0 as i64;
    while let Some((_, c)) = iter.peek() {
        if !c.is_ascii_digit() {
            // TODO: 0x
            break;
        }
        let d = c.to_digit(10).unwrap();
        value = value * 10 + d as i64;
        iter.next();
    }
    Token::Number(value)
}

/// 文字リテラルをパースする。'a' のような形式で、エスケープシーケンスも対応。
/// 呼び出し時点で開始の `'` は既に消費されている必要がある。
fn parse_char_literal(
    iter: &mut iter::Peekable<iter::Enumerate<impl Iterator<Item = char>>>,
    start_idx: usize,
) -> Result<Token, CodeParseError> {
    let char_value = match iter.next() {
        Some((_, '\\')) => {
            // エスケープシーケンス
            match iter.next() {
                Some((_, 'n')) => 10,   // 改行 (LF)
                Some((_, 'r')) => 13,   // 復帰 (CR)
                Some((_, 't')) => 9,    // タブ
                Some((_, 's')) => 32,   // スペース
                Some((_, '\\')) => 92,  // バックスラッシュ
                Some((_, '\'')) => 39,  // シングルクォート
                Some((idx, c)) => {
                    return Err(code_parse_error!(idx, format!("unknown escape sequence: \\{}", c)));
                }
                None => {
                    return Err(code_parse_error!(start_idx, "unexpected end of input in character literal".to_string()));
                }
            }
        }
        Some((_, '\'')) => {
            return Err(code_parse_error!(start_idx, "empty character literal".to_string()));
        }
        Some((_, c)) => c as i64,
        None => {
            return Err(code_parse_error!(start_idx, "unexpected end of input in character literal".to_string()));
        }
    };

    // 閉じる `'` を確認
    match iter.next() {
        Some((_, '\'')) => Ok(Token::Number(char_value)),
        Some((idx, c)) => {
            Err(code_parse_error!(idx, format!("expected closing quote, found: {}", c)))
        }
        None => {
            Err(code_parse_error!(start_idx, "unclosed character literal".to_string()))
        }
    }
}

fn determine_keyword_or_identifier(id: String) -> Token {
    match id.as_str() {
        "let" => Token::Keyword(Keyword::Let),
        "func" => Token::Keyword(Keyword::Func),
        "if" => Token::Keyword(Keyword::If),
        "else" => Token::Keyword(Keyword::Else),
        "while" => Token::Keyword(Keyword::While),
        "return" => Token::Keyword(Keyword::Return),
        "break" => Token::Keyword(Keyword::Break),
        "continue" => Token::Keyword(Keyword::Continue),
        _ => Token::Identifier(id),
    }
}

fn parse_identifier(
    iter: &mut iter::Peekable<iter::Enumerate<impl Iterator<Item = char>>>,
) -> Token {
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
            // return Token::Identifier(id);
            return determine_keyword_or_identifier(id);
        }
    }
}

fn parse_to_tokens_internal(
    iter: &mut iter::Peekable<iter::Enumerate<impl Iterator<Item = char>>>,
) -> (Vec<PrettyToken>, Vec<CodeParseError>) {
    let mut tokens = Vec::<PrettyToken>::new();
    let mut parse_errors = Vec::<CodeParseError>::new();
    while let Some((idx, c)) = iter.peek() {
        if *c == '#' {
            iter.next();
            while let Some((_, c2)) = iter.next() {
                if c2 == '#' {
                    break;
                }
            }
            continue;
        }
        let info = TokenInfo::new(*idx);
        if c.is_ascii_digit() {
            tokens.push((parse_number(iter), info));
        } else if c.is_whitespace() {
            iter.next();
            // c.is_ascii()
        } else {
            let t = match *c {
                'A'..='Z' | 'a'..='z' | '_' => {
                    tokens.push((parse_identifier(iter), info));
                    continue;
                }
                '\'' => {
                    // 文字リテラル
                    let start_idx = *idx;
                    iter.next(); // 開始の `'` を消費
                    match parse_char_literal(iter, start_idx) {
                        Ok(token) => {
                            tokens.push((token, info));
                            continue;
                        }
                        Err(err) => {
                            parse_errors.push(err);
                            continue;
                        }
                    }
                }
                '=' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::DoubleEqual,
                        _ => {
                            tokens.push((Token::SingleEqual, info));
                            continue;
                        }
                    }
                }
                '<' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::LessEqual,
                        _ => {
                            tokens.push((Token::Less, info));
                            continue;
                        }
                    }
                }
                '>' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::GreaterEqual,
                        _ => {
                            tokens.push((Token::Greater, info));
                            continue;
                        }
                    }
                }
                '!' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::NotEqual,
                        _ => {
                            tokens.push((Token::Exclamation, info));
                            continue;
                        }
                    }
                }
                '&' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '&' => Token::DoubleAmpersand,
                        _ => {
                            // 単独の & は未実装（参照演算子）
                            parse_errors.push(code_parse_error!(info.code_pointer, "single '&' is not supported yet".to_string()));
                            continue;
                        }
                    }
                }
                '|' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '|' => Token::DoublePipe,
                        _ => {
                            // 単独の | は未実装
                            parse_errors.push(code_parse_error!(info.code_pointer, "single '|' is not supported".to_string()));
                            continue;
                        }
                    }
                }
                '+' => Token::Plus,
                '-' => Token::Minus,
                '*' => Token::Asterisk,
                '/' => Token::Slash,
                '%' => Token::Percent,
                '(' => Token::ParenthesisL,
                ')' => Token::ParenthesisR,
                '[' => Token::BracketL,
                ']' => Token::BracketR,
                '{' => Token::BraceL,
                '}' => Token::BraceR,
                ';' => Token::Semicolon,
                ':' => Token::Colon,
                ',' => Token::Comma,
                _ => {
                    parse_errors.push(code_parse_error!(*idx, format!("invalid char: {}", c)));
                    iter.next();
                    continue;
                }
            };
            tokens.push((t, info));
            iter.next();
        }
    }
    (tokens, parse_errors)
}

pub fn parse_to_tokens(text: &str) -> Result<Vec<PrettyToken>, Vec<CodeParseError>> {
    let (tk, err) = parse_to_tokens_internal(&mut text.chars().enumerate().peekable());
    if err.is_empty() {
        Ok(tk)
    } else {
        Err(err)
    }
}

#[cfg(test)]
mod test;
