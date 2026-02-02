//! # Tree Parser
//!
//! このモジュールは、トークン列を木構造に変換します。
//! 式が存在するべき場所に式が存在するかどうか等の、構造上の文法の誤り等を検知します。
//!
use crate::base::CodeParseError;
use crate::token_parser::PrettyToken;

pub(crate) use self::expression::Expression;
pub(crate) use self::expression::Operator1;
pub(crate) use self::expression::Operator2;
use self::statement::parse_to_statements;
pub(crate) use self::statement::Statement;

#[macro_use]
mod macros;
mod expression;
mod statement;

// convert token sequence to tree structure.

pub fn parse_to_tree(tokens: &Vec<PrettyToken>) -> Result<Vec<Statement>, Vec<CodeParseError>> {
    let mut iter = tokens.iter().peekable();
    let (st, err) = parse_to_statements(&mut iter);
    if err.is_empty() {
        Ok(st)
    } else {
        Err(err)
    }
}
