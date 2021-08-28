use std::iter;

use crate::{
    base::CodeParseErrorInternal,
    code_parse_error,
    token_parser::{PrettyToken, Token, TokenInfo},
};

use super::expression::*;

//

// 期待するトークンなら Ok を返すマクロ
// そうでなければ、Expression::Invalid に渡すべき CodeParseError のインデックスを返す。
// NOTE: iter.next() or iter.peek()?
// peek は値を消費しない為に loop に陥る可能性がある。この判定は難しいので、next を推奨する。
macro_rules! match_expect_token {
    ($self: expr, $v: expr, $pat: pat) => {
        // #[warn(unused_must_use)]
        match $v {
            Some(($pat, _)) => Ok(()),
            Some((_, token_info)) => Err($self.add_parse_error(
                token_info,
                format!("unexpected token: expected {}", stringify!($pat)),
            )),
            None => Err($self.add_end_error("unexpected end of input".to_owned())),
        }
    };
    ($self: expr, $v: expr, $pat: pat if $cond:expr) => {
        // #[warn(unused_must_use)]
        match $v {
            Some(($pat, _)) if $cond => Ok(()),
            Some((_, token_info)) => Err($self.add_parse_error(
                token_info,
                format!("unexpected token: expected {}", stringify!($pat)),
            )),
            None => Err($self.add_end_error("unexpected end of input".to_owned())),
        }
    };
    ($self: expr, $v: expr, $pat: pat => $res: expr) => {
        match $v {
            Some(($pat, _)) => Ok($res),
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

    ($self: expr, $v: expr, $pat: pat if $cond:expr) => {
        let _ = match_expect_token!($self, $v, $pat if $cond);
    }
}

//

#[derive(Clone)] // TODO: REMOVE
pub enum Statement {
    VariableDeclaration(String, Box<Expression>),
    FunctionDeclaration(String, Vec<String>, Vec<Statement>),
    Continue,
    Break,
    Return(Box<Expression>),
    Expression(Box<Expression>),
    Invalid(usize), // See, Expression::Invalid
}

//

struct StatementBuilder<'b: 'a, 'a> {
    iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    code_parse_error: Vec<CodeParseErrorInternal>,
}

impl<'b: 'a, 'a> StatementBuilder<'b, 'a> {
    fn parse(
        iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    ) -> (Vec<Statement>, Vec<CodeParseErrorInternal>) {
        let mut b = Self {
            iter,
            code_parse_error: vec![],
        };
        let e = b.parse_to_statements();
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

    fn parse_to_statements_block(&mut self) -> Vec<Statement> {
        match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
        let ss = self.parse_to_statements();
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        return ss;
    }

    fn parse_to_statements_let(&mut self) -> Statement {
        if let Err(_) =
            match_expect_token!(self, self.iter.next(), Token::Identifier(id) if id == "let")
        {
            panic!("internal error");
        }
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        let id = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
            Ok(x) => x,
            Err(e) => {
                return Statement::Invalid(e);
            }
        };
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        return Statement::VariableDeclaration(id.clone(), Box::new(Expression::Factor(0)));
    }

    fn parse_to_statements_func(&mut self) -> Statement {
        if let Err(_) =
            match_expect_token!(self, self.iter.next(), Token::Identifier(id) if id == "func")
        {
            panic!("internal error");
        }
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        let id = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
            Ok(x) => x,
            Err(e) => {
                return Statement::Invalid(e);
            }
        };
        match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisL);
        let mut args = Vec::<String>::new();
        enum State {
            L,
            Var,
            Comma,
        }
        let mut state = State::L;
        loop {
            match self.iter.next() {
                Some((Token::Identifier(name), token_info)) => {
                    if let State::Var = state {
                        // note: 引数のparseに失敗するなら続行するべきではないと思う
                        self.add_parse_error(token_info, "expected ','".to_owned());
                    }
                    args.push(name.clone());
                    state = State::Var;
                }
                Some((Token::Comma, token_info)) => {
                    if let State::Var = state {
                        state = State::Comma;
                    } else {
                        self.add_parse_error(token_info, "unexpected ','".to_owned());
                    }
                }
                Some((Token::ParenthesisR, token_info)) => {
                    if let State::Comma = state {
                        self.add_parse_error(token_info, "unexpected ','".to_owned());
                    } else {
                        break;
                    }
                }
                Some((_, token_info)) => {
                    self.add_parse_error(token_info, "unexpected token".to_owned());
                    break;
                }
                None => {
                    self.add_end_error("unexpected end of input".to_owned());
                    break;
                }
            }
        }
        if let Err(e) = match_expect_token!(self, self.iter.peek(), Token::BraceL) {
            self.iter.next(); // NOTE: nextが安全だが不親切とは思う
            return Statement::Invalid(e);
        }
        return Statement::FunctionDeclaration(id.clone(), args, self.parse_to_statements_block());
    }

    fn parse_to_statements_return(&mut self) -> Statement {
        if let Err(_) =
            match_expect_token!(self, self.iter.next(), Token::Identifier(id) if id == "return")
        {
            panic!("internal error");
        }
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
        self.code_parse_error.append(&mut errs);
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        return Statement::Return(expr);
    }

    fn parse_to_statements(&mut self) -> Vec<Statement> {
        let mut statements = Vec::<Statement>::new();
        while let Some(token) = self.iter.peek() {
            match token {
                (Token::Identifier(identifier), _) => match identifier.as_str() {
                    "let" => {
                        statements.push(self.parse_to_statements_let());
                        continue;
                    }
                    "func" => {
                        statements.push(self.parse_to_statements_func());
                        continue;
                    }
                    "return" => {
                        statements.push(self.parse_to_statements_return());
                        continue;
                    }
                    "break" => {
                        self.iter.next();
                        statements.push(Statement::Break);
                        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                        continue;
                    }
                    "continue" => {
                        self.iter.next();
                        statements.push(Statement::Continue);
                        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                        continue;
                    }
                    _ => {}
                },
                (Token::BraceR, _) => {
                    // TODO: consider only BraceR
                    break;
                }
                _ => {}
            }
            let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
            self.code_parse_error.append(&mut errs);
            statements.push(Statement::Expression(expr));
            match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        }
        return statements;
        // panic!("syntax error: terminal");
    }
}

pub(super) fn parse_to_statements(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> (Vec<Statement>, Vec<CodeParseErrorInternal>) {
    StatementBuilder::parse(iter)
}
