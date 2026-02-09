use std::iter;

use crate::{
    base::{CodeParseError, SourceLocation},
    code_parse_error,
    token_parser::{Keyword, PrettyToken, Token, TokenInfo},
};

use super::expression::*;

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

#[derive(Clone)] // TODO: REMOVE
pub enum Statement {
    VariableDeclaration(String, Box<Expression>, bool), // (name, init_expr, is_static)
    FunctionDeclaration(String, Vec<String>, Vec<LocatedStatement>),
    Continue,
    Break,
    Return(Box<Expression>),
    Expression(Box<Expression>),
    Invalid(usize), // See, Expression::Invalid
}

/// 位置情報付きの Statement
#[derive(Clone)]
pub struct LocatedStatement {
    pub statement: Statement,
    pub location: SourceLocation,
}

//

struct StatementBuilder<'b: 'a, 'a> {
    iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    code_parse_error: Vec<CodeParseError>,
}

impl<'b: 'a, 'a> StatementBuilder<'b, 'a> {
    fn parse(
        iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    ) -> (Vec<LocatedStatement>, Vec<CodeParseError>) {
        let mut b = Self {
            iter,
            code_parse_error: vec![],
        };
        let e = b.parse_to_statements();
        (e, b.code_parse_error)
    }

    fn add_parse_error(
        &mut self,
        token_info: &TokenInfo,
        msg: impl Into<std::borrow::Cow<'static, str>>,
    ) -> usize {
        let i = self.code_parse_error.len();
        self.code_parse_error
            .push(code_parse_error!(token_info.code_pointer, msg));
        i
    }
    fn add_end_error(&mut self, msg: impl Into<std::borrow::Cow<'static, str>>) -> usize {
        let i = self.code_parse_error.len();
        self.code_parse_error.push(code_parse_error!(msg));
        i
    }

    fn parse_to_statements_block(&mut self) -> Vec<LocatedStatement> {
        match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
        let ss = self.parse_to_statements();
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        return ss;
    }

    fn parse_to_statements_let(&mut self, start_pos: usize) -> Vec<LocatedStatement> {
        if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Let)) {
            panic!("internal error");
        }
        self.parse_variable_declarations(start_pos, false)
    }

    fn parse_to_statements_static(&mut self, start_pos: usize) -> Vec<LocatedStatement> {
        if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Static))
        {
            panic!("internal error");
        }
        self.parse_variable_declarations(start_pos, true)
    }

    fn parse_variable_declarations(
        &mut self,
        start_pos: usize,
        is_static: bool,
    ) -> Vec<LocatedStatement> {
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);

        let mut results = Vec::<LocatedStatement>::new();

        loop {
            // 識別子を取得
            let id = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id)
            {
                Ok(x) => x,
                Err(e) => {
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(e),
                        location: SourceLocation::from_single(start_pos),
                    });
                    // エラーが発生したら残りをスキップしてセミコロンまで進む
                    while let Some((token, _)) = self.iter.peek() {
                        if matches!(token, Token::Semicolon) {
                            break;
                        }
                        self.iter.next();
                    }
                    break;
                }
            };

            // 初期化式のチェック
            let init_expr = if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
                // "(" を消費
                self.iter.next();

                // 初期化式をパース
                let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
                self.code_parse_error.append(&mut errs);

                // ")" を消費
                match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

                // 代入式を構築: id = expr
                Box::new(Expression::Operation2(
                    Operator2::Assign,
                    Box::new(Expression::Variable(id.clone())),
                    expr,
                ))
            } else {
                // 初期化式なし: デフォルトで 0
                Box::new(Expression::Factor(0))
            };

            let end_pos = self
                .iter
                .peek()
                .map(|(_, info)| info.code_pointer)
                .unwrap_or(start_pos);

            results.push(LocatedStatement {
                statement: Statement::VariableDeclaration(id.clone(), init_expr, is_static),
                location: SourceLocation::new(start_pos, end_pos),
            });

            // 次がカンマか確認
            if let Some((Token::Comma, _)) = self.iter.peek() {
                self.iter.next(); // カンマを消費
                continue; // 次の変数宣言へ
            } else {
                break; // カンマがなければループ終了
            }
        }

        // セミコロンを消費
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);

        results
    }

    fn parse_to_statements_func(&mut self, start_pos: usize) -> LocatedStatement {
        if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Func)) {
            panic!("internal error");
        }
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        let id = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
            Ok(x) => x,
            Err(e) => {
                return LocatedStatement {
                    statement: Statement::Invalid(e),
                    location: SourceLocation::from_single(start_pos),
                };
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
                        self.add_parse_error(token_info, "expected ','");
                    }
                    args.push(name.clone());
                    state = State::Var;
                }
                Some((Token::Comma, token_info)) => {
                    if let State::Var = state {
                        state = State::Comma;
                    } else {
                        self.add_parse_error(token_info, "unexpected ','");
                    }
                }
                Some((Token::ParenthesisR, token_info)) => {
                    if let State::Comma = state {
                        self.add_parse_error(token_info, "unexpected ','");
                    } else {
                        break;
                    }
                }
                Some((_, token_info)) => {
                    self.add_parse_error(token_info, "unexpected token");
                    break;
                }
                None => {
                    self.add_end_error("unexpected end of input");
                    break;
                }
            }
        }
        if let Err(e) = match_expect_token!(self, self.iter.peek(), Token::BraceL) {
            self.iter.next(); // NOTE: nextが安全だが不親切とは思う
            return LocatedStatement {
                statement: Statement::Invalid(e),
                location: SourceLocation::from_single(start_pos),
            };
        }
        let body = self.parse_to_statements_block();
        let end_pos = self
            .iter
            .peek()
            .map(|(_, info)| info.code_pointer)
            .unwrap_or(start_pos);
        return LocatedStatement {
            statement: Statement::FunctionDeclaration(id.clone(), args, body),
            location: SourceLocation::new(start_pos, end_pos),
        };
    }

    fn parse_to_statements_return(&mut self, start_pos: usize) -> LocatedStatement {
        if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Return))
        {
            panic!("internal error");
        }
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
        self.code_parse_error.append(&mut errs);
        let end_pos = self
            .iter
            .peek()
            .map(|(_, info)| info.code_pointer)
            .unwrap_or(start_pos);
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        return LocatedStatement {
            statement: Statement::Return(expr),
            location: SourceLocation::new(start_pos, end_pos),
        };
    }

    fn parse_to_statements(&mut self) -> Vec<LocatedStatement> {
        let mut statements = Vec::<LocatedStatement>::new();
        while let Some(token) = self.iter.peek() {
            let start_pos = token.1.code_pointer;
            match &token.0 {
                Token::Keyword(Keyword::Let) => {
                    statements.extend(self.parse_to_statements_let(start_pos));
                    continue;
                }
                Token::Keyword(Keyword::Static) => {
                    statements.extend(self.parse_to_statements_static(start_pos));
                    continue;
                }
                Token::Keyword(Keyword::Func) => {
                    statements.push(self.parse_to_statements_func(start_pos));
                    continue;
                }
                Token::Keyword(Keyword::Return) => {
                    statements.push(self.parse_to_statements_return(start_pos));
                    continue;
                }
                Token::Keyword(Keyword::Break) => {
                    self.iter.next();
                    let end_pos = self
                        .iter
                        .peek()
                        .map(|(_, info)| info.code_pointer)
                        .unwrap_or(start_pos);
                    statements.push(LocatedStatement {
                        statement: Statement::Break,
                        location: SourceLocation::new(start_pos, end_pos),
                    });
                    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                    continue;
                }
                Token::Keyword(Keyword::Continue) => {
                    self.iter.next();
                    let end_pos = self
                        .iter
                        .peek()
                        .map(|(_, info)| info.code_pointer)
                        .unwrap_or(start_pos);
                    statements.push(LocatedStatement {
                        statement: Statement::Continue,
                        location: SourceLocation::new(start_pos, end_pos),
                    });
                    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                    continue;
                }
                Token::BraceR => {
                    // TODO: consider only BraceR
                    break;
                }
                _ => {}
            }
            let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
            self.code_parse_error.append(&mut errs);
            let end_pos = self
                .iter
                .peek()
                .map(|(_, info)| info.code_pointer)
                .unwrap_or(start_pos);
            statements.push(LocatedStatement {
                statement: Statement::Expression(expr),
                location: SourceLocation::new(start_pos, end_pos),
            });
            match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        }
        return statements;
        // panic!("syntax error: terminal");
    }
}

pub(super) fn parse_to_statements(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> (Vec<LocatedStatement>, Vec<CodeParseError>) {
    StatementBuilder::parse(iter)
}

#[cfg(test)]
mod test;
