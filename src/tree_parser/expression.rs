use std::iter;

use crate::code_parse_error;

use crate::token_parser::{Keyword, TokenInfo};
use crate::tree_parser::statement::parse_to_statements;
use crate::{
    base::CodeParseErrorInternal,
    token_parser::{PrettyToken, Token},
};

use super::Statement;

//

// 期待するトークンなら Ok を返すマクロ
// そうでなければ、Expression::Invalid に渡すべき CodeParseError のインデックスを返す。
// NOTE: iter.next() or iter.peek()?
// peek は値を消費しない為に loop に陥る可能性がある。この判定は難しいので、next を推奨する。
macro_rules! match_expect_token {
    ($self: expr, $v: expr, $pat: pat) => {
        match $v {
            Some(($pat, _)) => Ok(()),
            Some((_, token_info)) => Err($self.add_parse_error(
                token_info,
                format!("unexpected token: expected {}", stringify!($pat)),
            )),
            None => Err($self.add_end_error("unexpected end of input".to_owned())),
        }
    };
}

// TODO: unused_must_use is experimental... remove this
macro_rules! match_expect_token_unused {
    ($self: expr, $v: expr, $pat: pat) => {
        let _ = match_expect_token!($self, $v, $pat);
    };
}

//

#[derive(Clone)] // TODO: REMOVE
pub enum Operator2 {
    Plus,
    Minus,
    Multiply,
    Divide,
    Assign,
}

#[derive(Clone)] // TODO: REMOVE
pub enum Expression {
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    If(Box<Expression>, Vec<Statement>, Vec<Statement>),
    While(Box<Expression>, Vec<Statement>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
    Invalid(usize), // NOTE: CodeParseError に関連する情報を入れる。今は CodeParseError の
                    // インデックスを利用。 本来は ExpressionBuilder 単位ではなく、全体で独立した
                    // インデックスを利用するべき。
                    // 構文木のノードからエラー情報を参照したい目的は特に無いので、使われていない。
}

//

struct ExpressionBuilder<'b: 'a, 'a> {
    iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    code_parse_error: Vec<CodeParseErrorInternal>,
}

impl<'b: 'a, 'a> ExpressionBuilder<'b, 'a> {
    fn parse(
        iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    ) -> (Box<Expression>, Vec<CodeParseErrorInternal>) {
        let mut b = Self {
            iter,
            code_parse_error: vec![],
        };
        let e = b.parse_to_expression_tree_root();
        (e, b.code_parse_error)
    }

    fn add_parse_error(&mut self, token_info: &TokenInfo, msg: String) -> usize {
        let i = self.code_parse_error.len();
        self.code_parse_error
            .push(code_parse_error!(token_info.code_pointer, msg.to_string()));
        i
    }
    fn add_end_error(&mut self, msg: String) -> usize {
        let i = self.code_parse_error.len();
        self.code_parse_error
            .push(code_parse_error!(0, msg.to_string()));
        i
    }

    fn parse_to_expression_tree_function(&mut self, name: &String) -> Box<Expression> {
        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::ParenthesisL) {
            return Box::new(Expression::Invalid(e));
        }

        let mut args = Vec::<Box<Expression>>::new();
        enum State {
            L,
            Eval,
            Comma,
        }
        let mut state = State::L;
        loop {
            match self.iter.peek() {
                Some((Token::ParenthesisR, token_info)) => {
                    if let State::Comma = state {
                        // weak syntax error and proceed parsing
                        self.add_parse_error(token_info, "unexpected comma".to_owned());
                    }
                    self.iter.next();
                    return Box::new(Expression::Function(name.clone(), args));
                }
                Some((Token::Comma, token_info)) => {
                    if let State::Eval = state {
                        state = State::Comma;
                    } else {
                        // weak syntax error and proceed parsing
                        self.add_parse_error(token_info, "unexpected comma".to_owned());
                    }
                    self.iter.next();
                }
                Some((_, token_info)) => {
                    if let State::Eval = state {
                        // weak syntax error and proceed parsing
                        self.add_parse_error(token_info, "missing comma".to_owned());
                    }
                    let e = self.parse_to_expression_tree_root();
                    args.push(e);
                    state = State::Eval;
                }
                None => {
                    return Box::new(Expression::Invalid(
                        self.add_end_error("unexpected end of input".to_owned()),
                    ))
                }
            }
        }
    }

    fn parse_to_expression_tree_factor(&mut self) -> Box<Expression> {
        match self.iter.peek() {
            Some((Token::Number(val), _)) => {
                self.iter.next();
                return Box::new(Expression::Factor(*val));
            }
            Some((Token::Identifier(id), _)) => {
                // TODO: confirm whether the identifier is reserved e.g. func
                self.iter.next();
                if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
                    return self.parse_to_expression_tree_function(id);
                }
                return Box::new(Expression::Variable(id.clone()));
            }
            Some((Token::ParenthesisL, _)) => {
                self.iter.next();
                let e = self.parse_to_expression_tree_root();

                if let Err(_) = match_expect_token!(self, self.iter.next(), Token::ParenthesisR) {
                    // weak syntax error and proceed parsing
                }
                return e;
            }
            Some((_, token_info)) => {
                return Box::new(Expression::Invalid(
                    self.add_parse_error(token_info, "unexpected token".to_owned()),
                ));
            }
            _ => {
                return Box::new(Expression::Invalid(
                    self.add_end_error("unexpected end of input".to_owned()),
                ));
            }
        }
    }

    fn parse_to_expression_tree_mul(&mut self) -> Box<Expression> {
        let mut left = self.parse_to_expression_tree_factor();
        loop {
            let op = if let Some(token) = self.iter.peek() {
                match token {
                    (Token::Symbol(chr), _) => match *chr {
                        '*' => Operator2::Multiply,
                        '/' => Operator2::Divide,
                        _ => return left,
                    },
                    _ => return left,
                }
            } else {
                return left;
            };
            self.iter.next();
            let right = self.parse_to_expression_tree_factor();
            left = Box::new(Expression::Operation2(op, left, right))
        }
    }

    fn parse_to_expression_tree_plus(&mut self) -> Box<Expression> {
        let mut left = self.parse_to_expression_tree_mul();
        loop {
            let op = if let Some(token) = self.iter.peek() {
                match token {
                    (Token::Symbol(chr), _) => match *chr {
                        '+' => Operator2::Plus,
                        '-' => Operator2::Minus,
                        _ => return left,
                    },
                    _ => return left,
                }
            } else {
                return left;
            };
            self.iter.next();
            let right = self.parse_to_expression_tree_mul();
            left = Box::new(Expression::Operation2(op, left, right));
        }
    }

    fn parse_to_expression_tree_assign(&mut self) -> Box<Expression> {
        let left = self.parse_to_expression_tree_plus();
        let op = if let Some(token) = self.iter.peek() {
            match token {
                (Token::Symbol(chr), _) if *chr == '=' => Operator2::Assign,
                _ => return left,
            }
        } else {
            return left;
        };
        self.iter.next();
        let right = self.parse_to_expression_tree_assign();
        Box::new(Expression::Operation2(op, left, right))
    }

    fn parse_to_expression_tree_while(&mut self) -> Box<Expression> {
        match self.iter.peek() {
            Some((Token::Keyword(Keyword::While), _)) => (),
            _ => return self.parse_to_expression_tree_assign(),
        }
        self.iter.next();

        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
            return Box::new(Expression::Invalid(e));
        }
        let cond = self.parse_to_expression_tree_root();
        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::BraceL) {
            return Box::new(Expression::Invalid(e));
        }
        let (stat, mut stat_err) = parse_to_statements(self.iter);
        if !stat_err.is_empty() {
            self.code_parse_error.append(&mut stat_err);
        }
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        Box::new(Expression::While(cond, stat))
    }

    fn parse_to_expression_tree_if(&mut self) -> Box<Expression> {
        match self.iter.peek() {
            Some((Token::Keyword(Keyword::If), _)) => (),
            _ => return self.parse_to_expression_tree_while(),
        };
        self.iter.next();

        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
            return Box::new(Expression::Invalid(e));
        }
        let cond = self.parse_to_expression_tree_root();
        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::BraceL) {
            // NOTE: statements ではなく expression が来ても許容、でいいかもね？
            return Box::new(Expression::Invalid(e));
        }

        let (stats_true, mut stats_err) = parse_to_statements(self.iter);
        if !stats_err.is_empty() {
            self.code_parse_error.append(&mut stats_err);
        }

        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);

        let stats_false = match self.iter.peek() {
            Some((Token::Keyword(Keyword::Else), _)) => {
                self.iter.next();
                match_expect_token_unused!(self, self.iter.next(), Token::Colon);

                match self.iter.peek() {
                    Some((Token::Keyword(Keyword::If), _)) => {
                        // else: if: cond {}
                        // TODO: elsif を実装したほうが便利？
                        // TODO: allow single expression ???
                        vec![Statement::Expression(self.parse_to_expression_tree_if())]
                    }
                    _ => {
                        let (stats, mut stats_err) = parse_to_statements(self.iter);
                        if !stats_err.is_empty() {
                            self.code_parse_error.append(&mut stats_err);
                        }
                        stats
                    }
                }
            }
            _ => {
                vec![]
            }
        };
        Box::new(Expression::If(cond, stats_true, stats_false))
    }

    fn parse_to_expression_tree_root(&mut self) -> Box<Expression> {
        // TODO: check the expression that it has Invalid
        self.parse_to_expression_tree_if()
    }
}

pub(super) fn parse_to_expression_tree_root(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> (Box<Expression>, Vec<CodeParseErrorInternal>) {
    ExpressionBuilder::parse(iter)
}
