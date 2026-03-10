//! # Tree Parser
//!
//! このモジュールは、トークン列を木構造に変換します。
//! 式が存在するべき場所に式が存在するかどうか等の、構造上の文法の誤り等を検知します。
//!
use crate::base::CodeParseError;
use crate::code_parse_error;
use crate::token_parser::PrettyToken;

pub(crate) use self::expression::Expression;
pub(crate) use self::expression::LocatedExpression;
pub(crate) use self::expression::Operator1;
pub(crate) use self::expression::Operator2;
pub(crate) use self::expression::TypeSpec;
use self::statement::parse_to_statements;
pub(crate) use self::statement::{
    AliasArg, AliasParam, AliasParamKind, LocatedStatement, Statement, StructFieldDecl,
};

#[macro_use]
mod macros;
mod expression;
mod statement;

/// `stringify!()` で生成される期待トークンパターン文字列を人間可読な形式に変換する
pub(super) fn describe_expected_token(pat: &str) -> &str {
    match pat {
        "Token::Semicolon" => "';'",
        "Token::Colon" => "':'",
        "Token::Comma" => "','",
        "Token::ParenthesisL" => "'('",
        "Token::ParenthesisR" => "')'",
        "Token::BracketL" => "'['",
        "Token::BracketR" => "']'",
        "Token::BraceL" => "'{'",
        "Token::BraceR" => "'}'",
        "Token::SingleEqual" => "'='",
        "Token::Identifier(id)" | "Token::Identifier(_)" => "identifier",
        "Token::Number(_)" => "number",
        _ => pat, // フォールバック: そのまま表示
    }
}

// convert token sequence to tree structure.

pub fn parse_to_tree(
    tokens: &Vec<PrettyToken>,
) -> Result<Vec<LocatedStatement>, Vec<CodeParseError>> {
    let mut iter = tokens.iter().peekable();
    let (st, mut err) = parse_to_statements(&mut iter);

    // 余剰トークンのチェック
    if let Some((token, token_info)) = iter.next() {
        err.push(code_parse_error!(
            token_info.code_pointer,
            format!(
                "unexpected token {} (unmatched closing brace or extra code)",
                token.describe()
            )
        ));
    }

    if err.is_empty() {
        Ok(st)
    } else {
        Err(err)
    }
}
