use std::{
    iter::{self, Enumerate, Peekable},
    str::Chars,
};

use crate::{
    base::CodeParseError,
    token_parser::{parse_to_tokens_internal, Keyword, Token},
};

use super::PrettyToken;

fn res_parse_to_tokens_internal(
    iter: &mut iter::Peekable<iter::Enumerate<Chars>>,
) -> Result<Vec<PrettyToken>, Vec<CodeParseError>> {
    let (tk, err) = parse_to_tokens_internal(iter);

    if err.is_empty() {
        Ok(tk)
    } else {
        Err(err)
    }
}

fn to_iter(code: &str) -> Peekable<Enumerate<Chars<'_>>> {
    code.chars().enumerate().peekable()
}

macro_rules! test_ok_parse_single {
    ($name: ident, $val: expr, $($pat:pat_param)|+ if $cond:expr ) => {
        // note: concat_idents! is only for nightly
        #[test]
        fn $name() -> Result<(), &'static str> {
            let res = res_parse_to_tokens_internal(&mut to_iter($val)).unwrap();
            let mut it = res.iter();
            if let Some((token, _)) = it.next() {
                assert_matches!(token, $($pat)|+ if $cond);
                Ok(())
            } else {
                Err("no token are parsed")
            }
        }
    };
    ($name: ident, $val: expr, $($pat:pat_param)|+ ) => {
        // note: concat_idents! is only for nightly
        #[test]
        fn $name() -> Result<(), &'static str> {
            let res = res_parse_to_tokens_internal(&mut to_iter($val)).unwrap();
            let mut it = res.iter();
            if let Some((token, _)) = it.next() {
                assert_matches!(token, $($pat)|+);
                Ok(())
            } else {
                Err("no token are parsed")
            }
        }
    };
}

macro_rules! test_ok_parse {
    ($name: ident, $val: expr, $it: ident => $block: block) => {
        // note: concat_idents! is only for nightly
        #[test]
        fn $name() {
            let res = res_parse_to_tokens_internal(&mut to_iter($val)).unwrap();
            {
                let mut $it = res.iter().map(|pt| &pt.0);
                $block
            }
        }
    };
}

macro_rules! test_ok_parse_number {
    ($name: ident, $val: expr) => {
        test_ok_parse_single!($name, stringify!($val), Token::Number(n) if *n == $val);
    };
}

macro_rules! test_ok_parse_identifier {
    ($name: ident, $val: expr) => {
        test_ok_parse_single!($name, $val, Token::Identifier(id) if id == $val);
    };
}

test_ok_parse_number!(test_ok_pn_1, 50);
test_ok_parse_identifier!(test_ok_pi_1, "sushi123");
test_ok_parse_identifier!(test_ok_pi_2, "MOCHI_");
test_ok_parse_identifier!(test_ok_pi_3, "__uni__");
test_ok_parse_identifier!(test_ok_pi_4, "_998244353");

test_ok_parse!(test_ok_p_1, "2+3", it => {
    assert_matches!(it.next(), Some(Token::Number(n)) if *n == 2);
    assert_matches!(it.next(), Some(Token::Plus));
    assert_matches!(it.next(), Some(Token::Number(n)) if *n == 3);
    assert_matches!(it.next(), None);
});

test_ok_parse!(test_ok_p_2, "let:a;", it => {
    // "let:" は一つの Keyword トークンとしてコロンを内包みになる
    assert_matches!(it.next(), Some(Token::Keyword(Keyword::Let)));
    assert_matches!(it.next(), Some(Token::Identifier(x)) if *x == "a");
    assert_matches!(it.next(), Some(Token::Semicolon));
    assert_matches!(it.next(), None);
});

// コロンなしのキーワード候補は識別子としてパースされる
test_ok_parse_identifier!(test_ok_pi_keyword_let_no_colon, "let");
test_ok_parse_identifier!(test_ok_pi_keyword_break_no_colon, "break");
test_ok_parse_identifier!(test_ok_pi_keyword_continue_no_colon, "continue");
test_ok_parse_identifier!(test_ok_pi_keyword_return_no_colon, "return");

// break:; → [Keyword(Break), Semicolon]
test_ok_parse!(test_ok_break_colon_semicolon, "break:;", it => {
    assert_matches!(it.next(), Some(Token::Keyword(Keyword::Break)));
    assert_matches!(it.next(), Some(Token::Semicolon));
    assert_matches!(it.next(), None);
});

// continue:; → [Keyword(Continue), Semicolon]
test_ok_parse!(test_ok_continue_colon_semicolon, "continue:;", it => {
    assert_matches!(it.next(), Some(Token::Keyword(Keyword::Continue)));
    assert_matches!(it.next(), Some(Token::Semicolon));
    assert_matches!(it.next(), None);
});

// return:; → [Keyword(Return), Semicolon]
test_ok_parse!(test_ok_return_colon_semicolon, "return:;", it => {
    assert_matches!(it.next(), Some(Token::Keyword(Keyword::Return)));
    assert_matches!(it.next(), Some(Token::Semicolon));
    assert_matches!(it.next(), None);
});

// コロンなしの break/continue は識別子としてパースされる
test_ok_parse!(test_ok_break_no_colon_is_ident, "break;", it => {
    assert_matches!(it.next(), Some(Token::Identifier(x)) if *x == "break");
    assert_matches!(it.next(), Some(Token::Semicolon));
    assert_matches!(it.next(), None);
});

// Error case tests

#[test]
fn test_fail_unclosed_char_literal() {
    let result = res_parse_to_tokens_internal(&mut to_iter("'a"));
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_fail_empty_char_literal() {
    let result = res_parse_to_tokens_internal(&mut to_iter("''"));
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_fail_invalid_escape_sequence() {
    let result = res_parse_to_tokens_internal(&mut to_iter("'\\x'"));
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_fail_unclosed_char_literal_with_escape() {
    let result = res_parse_to_tokens_internal(&mut to_iter("'\\n"));
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_fail_char_literal_too_many_chars() {
    let result = res_parse_to_tokens_internal(&mut to_iter("'ab'"));
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

// Character literal and escape sequence tests

test_ok_parse_single!(test_char_literal_simple, "'a'", Token::Number(n) if *n == 97);
test_ok_parse_single!(test_char_literal_newline, "'\\n'", Token::Number(n) if *n == 10);
test_ok_parse_single!(test_char_literal_tab, "'\\t'", Token::Number(n) if *n == 9);
test_ok_parse_single!(test_char_literal_carriage_return, "'\\r'", Token::Number(n) if *n == 13);
test_ok_parse_single!(test_char_literal_space, "'\\s'", Token::Number(n) if *n == 32);
test_ok_parse_single!(test_char_literal_backslash, "'\\\\'", Token::Number(n) if *n == 92);
test_ok_parse_single!(test_char_literal_quote, "'\\''", Token::Number(n) if *n == 39);

// Hex escape sequence tests (variable-length)
test_ok_parse_single!(test_char_literal_hex_2digit, "'\\x41'", Token::Number(n) if *n == 65); // 'A'
test_ok_parse_single!(test_char_literal_hex_2digit_ff, "'\\xFF'", Token::Number(n) if *n == 255);
test_ok_parse_single!(test_char_literal_hex_4digit, "'\\xFF03'", Token::Number(n) if *n == 0xFF03); // 65283
test_ok_parse_single!(test_char_literal_hex_5digit, "'\\x1F363'", Token::Number(n) if *n == 0x1F363); // 127843 (🍣)

#[test]
fn test_fail_hex_escape_too_few_digits() {
    // \x の後に1桁しかない場合はエラー
    let result = res_parse_to_tokens_internal(&mut to_iter("'\\xF'"));
    assert!(result.is_err());
}

// Empty input test
#[test]
fn test_empty_input() {
    let result = res_parse_to_tokens_internal(&mut to_iter(""));
    assert!(result.is_ok());
    let tokens = result.unwrap();
    assert_eq!(tokens.len(), 0);
}

// Reference operator tests
test_ok_parse_single!(test_ampersand_single, "&", Token::Ampersand);

test_ok_parse!(test_ampersand_and_identifier, "&x", it => {
    assert_matches!(it.next(), Some(Token::Ampersand));
    assert_matches!(it.next(), Some(Token::Identifier(x)) if *x == "x");
    assert_matches!(it.next(), None);
});

test_ok_parse!(test_double_ampersand, "&&", it => {
    assert_matches!(it.next(), Some(Token::DoubleAmpersand));
    assert_matches!(it.next(), None);
});

// Integer overflow tests
#[test]
fn test_integer_overflow_decimal() {
    // i64::MAX = 9223372036854775807 なので、それより大きい数はエラー
    let result = res_parse_to_tokens_internal(&mut to_iter("99999999999999999999"));
    assert!(result.is_err());
}

#[test]
fn test_integer_overflow_hex() {
    // 0x の後に大きすぎる16進数
    let result = res_parse_to_tokens_internal(&mut to_iter("0xFFFFFFFFFFFFFFFFFF"));
    assert!(result.is_err());
}

#[test]
fn test_integer_max_value() {
    // i64::MAX は正常にパースできるべき
    let result = res_parse_to_tokens_internal(&mut to_iter("9223372036854775807"));
    assert!(result.is_ok());
    let tokens = result.unwrap();
    assert_eq!(tokens.len(), 1);
    assert_matches!(&tokens[0].0, Token::Number(n) if *n == i64::MAX);
}
