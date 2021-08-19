use std::{
    iter::{Enumerate, Peekable},
    str::Chars,
};

use crate::token_parser::{parse_identifier, parse_number, parse_to_tokens_internal, Token};

fn to_iter(code: &str) -> Peekable<Enumerate<Chars>> {
    code.chars().enumerate().peekable()
}

macro_rules! test_ok_parse_number {
    ($name: ident, $val: expr) => {
        // note: concat_idents! is only for nightly
        #[test]
        fn $name() {
            assert_matches!(
                parse_number(&mut to_iter(stringify!($val))),
                Token::Number($val)
            );
        }
    };
}

macro_rules! test_ok_parse_identifier {
    ($name: ident, $val: expr) => {
        // note: concat_idents! is only for nightly
        #[test]
        fn $name() {
            assert_matches!(
                parse_identifier(&mut to_iter($val)),
                Token::Identifier(id) if id == $val
            );
        }
    };
}

macro_rules! test_ok_parse {
    ($name: ident, $val: expr, $it: ident => $block: tt) => {
        // note: concat_idents! is only for nightly
        #[test]
        fn $name() {
            let res = parse_to_tokens_internal(&mut to_iter($val));
            {
                let mut $it = res.iter().map(|pt| &pt.0);
                $block
            }
        }
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
    assert_matches!(it.next(), Some(Token::Identifier(x)) if *x == "let");
    assert_matches!(it.next(), Some(Token::Colon));
    assert_matches!(it.next(), Some(Token::Identifier(x)) if *x == "a");
    assert_matches!(it.next(), Some(Token::Semicolon));
    assert_matches!(it.next(), None);
});

// TODO: add fail case
// TODO: add tokeninfo
