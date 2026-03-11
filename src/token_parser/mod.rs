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
    For,
    Repeat,
    Return,
    Break,
    Continue,
    Static,
    Constexpr,
    Alias,
    Final,
    Namespace,
    Import,
    Weak,
    Export,
    Struct,
}

#[derive(Debug)]
pub enum Token {
    Number(i64),
    Identifier(String),
    Keyword(Keyword),
    StringLiteral(Vec<i64>), // 文字列リテラル（各文字のASCII値のベクタ、ヌル終端は含まない）
    At,                      // @
    Dot,                     // .
    Plus,
    Minus,
    Asterisk,
    Slash,
    Percent, // %
    Exclamation,
    SingleEqual,
    DoubleEqual,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    PlusEqual,       // +=
    MinusEqual,      // -=
    AsteriskEqual,   // *=
    SlashEqual,      // /=
    PercentEqual,    // %=
    DoubleAmpersand, // &&
    DoublePipe,      // ||
    Ampersand,       // &
    ParenthesisL,    // (
    ParenthesisR,    // )
    BracketL,        // [
    BracketR,        // ]
    BraceL,          // {
    BraceR,          // }
    Semicolon,       // ;
    Colon,           // :
    Comma,           // ,
    Dollar,          // $
    Invalid,
}

#[derive(Debug)]
pub struct TokenInfo {
    pub code_pointer: usize,
}

pub type PrettyToken = (Token, TokenInfo);

impl TokenInfo {
    fn new(code_pointer: usize) -> Self {
        TokenInfo { code_pointer }
    }
}

impl Keyword {
    /// エラーメッセージ用にキーワードの文字列表現を返す
    pub fn as_str(&self) -> &'static str {
        match self {
            Keyword::Let => "let",
            Keyword::Func => "func",
            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::While => "while",
            Keyword::For => "for",
            Keyword::Repeat => "repeat",
            Keyword::Return => "return",
            Keyword::Break => "break",
            Keyword::Continue => "continue",
            Keyword::Static => "static",
            Keyword::Constexpr => "constexpr",
            Keyword::Alias => "alias",
            Keyword::Final => "final",
            Keyword::Namespace => "namespace",
            Keyword::Import => "import",
            Keyword::Weak => "weak",
            Keyword::Export => "export",
            Keyword::Struct => "struct",
        }
    }
}

impl Token {
    /// エラーメッセージ用の人間可読な説明を返す
    pub fn describe(&self) -> String {
        match self {
            Token::Number(n) => format!("number '{}'", n),
            Token::Identifier(s) => format!("identifier '{}'", s),
            Token::Keyword(k) => format!("keyword '{}'", k.as_str()),
            Token::StringLiteral(_) => "string literal".to_string(),
            Token::At => "'@'".to_string(),
            Token::Dot => "'.'".to_string(),
            Token::Plus => "'+'".to_string(),
            Token::Minus => "'-'".to_string(),
            Token::Asterisk => "'*'".to_string(),
            Token::Slash => "'/'".to_string(),
            Token::Percent => "'%'".to_string(),
            Token::Exclamation => "'!'".to_string(),
            Token::SingleEqual => "'='".to_string(),
            Token::DoubleEqual => "'=='".to_string(),
            Token::NotEqual => "'!='".to_string(),
            Token::Less => "'<'".to_string(),
            Token::Greater => "'>'".to_string(),
            Token::LessEqual => "'<='".to_string(),
            Token::GreaterEqual => "'>='".to_string(),
            Token::PlusEqual => "'+='".to_string(),
            Token::MinusEqual => "'-='".to_string(),
            Token::AsteriskEqual => "'*='".to_string(),
            Token::SlashEqual => "'/='".to_string(),
            Token::PercentEqual => "'%='".to_string(),
            Token::DoubleAmpersand => "'&&'".to_string(),
            Token::DoublePipe => "'||'".to_string(),
            Token::Ampersand => "'&'".to_string(),
            Token::ParenthesisL => "'('".to_string(),
            Token::ParenthesisR => "')'".to_string(),
            Token::BracketL => "'['".to_string(),
            Token::BracketR => "']'".to_string(),
            Token::BraceL => "'{'".to_string(),
            Token::BraceR => "'}'".to_string(),
            Token::Semicolon => "';'".to_string(),
            Token::Colon => "':'".to_string(),
            Token::Comma => "','".to_string(),
            Token::Dollar => "'$'".to_string(),
            Token::Invalid => "invalid token".to_string(),
        }
    }
}

fn parse_number<I: Iterator<Item = (usize, char)>>(
    iter: &mut iter::Peekable<I>,
) -> Result<Token, CodeParseError> {
    // token レベルでは負の数を扱うことはできない

    // 最初の文字をチェック
    let first_char = iter.peek();
    if let Some((idx, '0')) = first_char {
        let hex_idx = *idx;
        iter.next(); // '0' を消費

        // 次の文字が 'x' または 'X' なら16進数
        if let Some((_, 'x')) | Some((_, 'X')) = iter.peek() {
            iter.next(); // 'x' または 'X' を消費

            // 16進数をパース
            let mut value = 0i64;
            let mut has_digit = false;
            while let Some((idx, c)) = iter.peek() {
                if let Some(d) = c.to_digit(16) {
                    let idx = *idx;
                    value = value
                        .checked_mul(16)
                        .and_then(|v| v.checked_add(d as i64))
                        .ok_or_else(|| code_parse_error!(idx, "integer literal overflow"))?;
                    has_digit = true;
                    iter.next();
                } else {
                    break;
                }
            }

            if !has_digit {
                return Err(code_parse_error!(
                    hex_idx,
                    "invalid hexadecimal literal: expected at least one hex digit after '0x'"
                ));
            }

            return Ok(Token::Number(value));
        }
        // '0' の後に 'x' がない場合は '0' として開始し、後続の10進数を処理
        // value は既に 0 なので、そのまま10進数パースに進む
    }

    // 10進数をパース
    let mut value = 0i64;
    while let Some((idx, c)) = iter.peek() {
        if !c.is_ascii_digit() {
            break;
        }
        let idx = *idx;
        let d = c.to_digit(10).unwrap();
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(d as i64))
            .ok_or_else(|| code_parse_error!(idx, "integer literal overflow"))?;
        iter.next();
    }
    Ok(Token::Number(value))
}

/// 16進数エスケープシーケンスをパースする。
/// `\x` の直後から呼び出し、16進数文字を貪欲に読み取る（最低2桁必要）。
fn parse_hex_escape<I: Iterator<Item = (usize, char)>>(
    iter: &mut iter::Peekable<I>,
    x_idx: usize,
) -> Result<i64, CodeParseError> {
    let mut hex_str = String::new();

    // 16進数文字を貪欲に読み取る
    while let Some(&(_, c)) = iter.peek() {
        if c.is_ascii_hexdigit() {
            hex_str.push(c);
            iter.next();
        } else {
            break;
        }
    }

    if hex_str.len() < 2 {
        return Err(code_parse_error!(
            x_idx,
            "incomplete hex escape sequence: expected at least 2 hex digits after '\\x'"
        ));
    }

    i64::from_str_radix(&hex_str, 16).map_err(|_| {
        code_parse_error!(
            x_idx,
            format!("invalid hex escape sequence: \\x{}", hex_str)
        )
    })
}

/// エスケープシーケンスをパースする共通関数
///
/// `\` の次の文字から処理を開始する。`iter` は `\` の次を指している状態で呼ぶこと。
/// `context` はエラーメッセージ用（"character literal" or "string literal"）。
fn parse_escape_sequence<I: Iterator<Item = (usize, char)>>(
    iter: &mut iter::Peekable<I>,
    start_idx: usize,
    context: &str,
) -> Result<i64, CodeParseError> {
    match iter.next() {
        Some((_, 'n')) => Ok(10),  // 改行 (LF)
        Some((_, 'r')) => Ok(13),  // 復帰 (CR)
        Some((_, 't')) => Ok(9),   // タブ
        Some((_, 's')) => Ok(32),  // スペース
        Some((_, '\\')) => Ok(92), // バックスラッシュ
        Some((_, '"')) => Ok(34),  // ダブルクォート
        Some((_, '\'')) => Ok(39), // シングルクォート
        Some((idx, 'x')) => {
            // 16進数エスケープシーケンス \xHH...（可変長、最低2桁）
            parse_hex_escape(iter, idx)
        }
        Some((idx, c)) => Err(code_parse_error!(
            idx,
            format!("unknown escape sequence: \\{}", c)
        )),
        None => Err(code_parse_error!(
            start_idx,
            format!("unexpected end of input in {}", context)
        )),
    }
}

/// 文字リテラルをパースする。'a' のような形式で、エスケープシーケンスも対応。
/// 呼び出し時点で開始の `'` は既に消費されている必要がある。
fn parse_char_literal<I: Iterator<Item = (usize, char)>>(
    iter: &mut iter::Peekable<I>,
    start_idx: usize,
) -> Result<Token, CodeParseError> {
    let char_value = match iter.next() {
        Some((_, '\\')) => {
            // エスケープシーケンス
            parse_escape_sequence(iter, start_idx, "character literal")?
        }
        Some((_, '\'')) => {
            return Err(code_parse_error!(start_idx, "empty character literal"));
        }
        Some((_, c)) => c as i64,
        None => {
            return Err(code_parse_error!(
                start_idx,
                "unexpected end of input in character literal"
            ));
        }
    };

    // 閉じる `'` を確認
    match iter.next() {
        Some((_, '\'')) => Ok(Token::Number(char_value)),
        Some((idx, c)) => Err(code_parse_error!(
            idx,
            format!("expected closing quote, found: {}", c)
        )),
        None => Err(code_parse_error!(start_idx, "unclosed character literal")),
    }
}

/// 文字列リテラルをパースする。"..." のような形式で、エスケープシーケンスも対応。
/// 呼び出し時点で開始の `"` は既に消費されている必要がある。
/// ヌル終端は含まない（tree_parser で追加される）。
fn parse_string_literal<I: Iterator<Item = (usize, char)>>(
    iter: &mut iter::Peekable<I>,
    start_idx: usize,
) -> Result<Token, CodeParseError> {
    let mut chars = Vec::new();

    loop {
        match iter.peek() {
            Some((_, '"')) => {
                // 終了のダブルクォート
                iter.next();
                return Ok(Token::StringLiteral(chars));
            }
            Some((_, '\\')) => {
                // エスケープシーケンス
                iter.next(); // '\\' を消費
                let value = parse_escape_sequence(iter, start_idx, "string literal")?;
                chars.push(value);
            }
            Some((_, c)) => {
                // 通常の文字（空白文字も含む - nospace では空白は無視されるが、文字列内では保持）
                chars.push(*c as i64);
                iter.next();
            }
            None => {
                return Err(code_parse_error!(
                    start_idx,
                    "unclosed string literal: expected closing '\"'"
                ));
            }
        }
    }
}

/// キーワード候補の文字列を Keyword トークンに変換する。
/// コロン付きの場合のみキーワードとして認識するため、コロン確認後に呼び出すこと。
fn as_keyword_token(id: &str) -> Option<Token> {
    match id {
        "let" => Some(Token::Keyword(Keyword::Let)),
        "func" => Some(Token::Keyword(Keyword::Func)),
        "if" => Some(Token::Keyword(Keyword::If)),
        "else" => Some(Token::Keyword(Keyword::Else)),
        "while" => Some(Token::Keyword(Keyword::While)),
        "for" => Some(Token::Keyword(Keyword::For)),
        "repeat" => Some(Token::Keyword(Keyword::Repeat)),
        "return" => Some(Token::Keyword(Keyword::Return)),
        "break" => Some(Token::Keyword(Keyword::Break)),
        "continue" => Some(Token::Keyword(Keyword::Continue)),
        "static" => Some(Token::Keyword(Keyword::Static)),
        "constexpr" => Some(Token::Keyword(Keyword::Constexpr)),
        "alias" => Some(Token::Keyword(Keyword::Alias)),
        "final" => Some(Token::Keyword(Keyword::Final)),
        "namespace" => Some(Token::Keyword(Keyword::Namespace)),
        "import" => Some(Token::Keyword(Keyword::Import)),
        "weak" => Some(Token::Keyword(Keyword::Weak)),
        "export" => Some(Token::Keyword(Keyword::Export)),
        "struct" => Some(Token::Keyword(Keyword::Struct)),
        _ => None,
    }
}

/// 識別子またはキーワードをパースする。
///
/// キーワードの直後にコロン (`:`) が続く場合のみ Keyword トークンを返す（コロンを内包して消費）。
/// コロンが続かない場合は Identifier トークンを返す。これにより `let` 等の予約語をユーザー変数名として使用可能になる。
fn parse_identifier<I: Iterator<Item = (usize, char)>>(iter: &mut iter::Peekable<I>) -> Token {
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
            // キーワード候補の場合、コロンが続く場合のみ Keyword トークンとして扱う
            // コロンを内包（消費）することで、後段パーサーがコロンを期待しない設計になる
            if let Some((_, ':')) = iter.peek() {
                if let Some(kw) = as_keyword_token(&id) {
                    iter.next(); // ':' を消費（Keyword トークンに内包）
                    return kw;
                }
            }
            return Token::Identifier(id);
        }
    }
}

fn parse_to_tokens_internal<I: Iterator<Item = (usize, char)>>(
    iter: &mut iter::Peekable<I>,
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
            match parse_number(iter) {
                Ok(token) => {
                    tokens.push((token, info));
                }
                Err(err) => {
                    parse_errors.push(err);
                }
            }
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
                '"' => {
                    // 文字列リテラル
                    let start_idx = *idx;
                    iter.next(); // 開始の `"` を消費
                    match parse_string_literal(iter, start_idx) {
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
                            // 単独の & は参照演算子
                            tokens.push((Token::Ampersand, info));
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
                            parse_errors.push(code_parse_error!(
                                info.code_pointer,
                                "single '|' is not supported"
                            ));
                            continue;
                        }
                    }
                }
                '+' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::PlusEqual,
                        _ => {
                            tokens.push((Token::Plus, info));
                            continue;
                        }
                    }
                }
                '-' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::MinusEqual,
                        _ => {
                            tokens.push((Token::Minus, info));
                            continue;
                        }
                    }
                }
                '*' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::AsteriskEqual,
                        _ => {
                            tokens.push((Token::Asterisk, info));
                            continue;
                        }
                    }
                }
                '/' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::SlashEqual,
                        _ => {
                            tokens.push((Token::Slash, info));
                            continue;
                        }
                    }
                }
                '%' => {
                    iter.next();
                    match iter.peek() {
                        Some((_, c)) if *c == '=' => Token::PercentEqual,
                        _ => {
                            tokens.push((Token::Percent, info));
                            continue;
                        }
                    }
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
                '$' => Token::Dollar,
                '@' => Token::At,
                '.' => Token::Dot,
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
    // Remove whitespace characters completely!!!
    let (tk, err) = parse_to_tokens_internal(
        &mut text
            .chars()
            .enumerate()
            .filter(|(_, c)| !c.is_whitespace())
            .peekable(),
    );
    if err.is_empty() {
        Ok(tk)
    } else {
        Err(err)
    }
}

#[cfg(test)]
mod test;
