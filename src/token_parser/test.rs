use std::{
    iter::{self, Enumerate, Peekable},
    str::Chars,
};

use crate::{
    base::CodeParseErrorInternal,
    token_parser::{parse_to_tokens_internal, Keyword, Token},
};

use super::PrettyToken;

fn res_parse_to_tokens_internal(
    iter: &mut iter::Peekable<iter::Enumerate<Chars>>,
) -> Result<Vec<PrettyToken>, Vec<CodeParseErrorInternal>> {
    let (tk, err) = parse_to_tokens_internal(iter);

    if err.is_empty() {
        Ok(tk)
    } else {
        Err(err)
    }
}

fn to_iter(code: &str) -> Peekable<Enumerate<Chars>> {
    code.chars().enumerate().peekable()
}

macro_rules! test_ok_parse_single {
    ($name: ident, $val: expr, $($pat:pat)|+ if $cond:expr ) => {
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
    ($name: ident, $val: expr, $($pat:pat)|+ ) => {
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
    assert_matches!(it.next(), Some(Token::Symbol(s)) if *s == '+');
    assert_matches!(it.next(), Some(Token::Number(n)) if *n == 3);
    assert_matches!(it.next(), None);
});

test_ok_parse!(test_ok_p_2, "let:a;", it => {
    assert_matches!(it.next(), Some(Token::Keyword(Keyword::Let)));
    assert_matches!(it.next(), Some(Token::Colon));
    assert_matches!(it.next(), Some(Token::Identifier(x)) if *x == "a");
    assert_matches!(it.next(), Some(Token::Semicolon));
    assert_matches!(it.next(), None);
});

// TODO: add fail case
// TODO: add tokeninfo
