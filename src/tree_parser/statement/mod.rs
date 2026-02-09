use std::iter;

use crate::{
    base::{CodeParseError, SourceLocation},
    code_parse_error,
    token_parser::{Keyword, PrettyToken, Token, TokenInfo},
};

use super::expression::*;

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

#[derive(Clone, Debug)] // TODO: REMOVE
pub enum Statement {
    VariableDeclaration(String, Box<Expression>, bool, Option<i64>), // (name, init_expr, is_static, array_size)
    FunctionDeclaration(String, Vec<String>, Vec<LocatedStatement>),
    Continue,
    Break,
    Return(Box<Expression>),
    Expression(Box<Expression>),
    Invalid(usize), // See, Expression::Invalid
}

/// 位置情報付きの Statement
#[derive(Clone, Debug)]
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

            // 配列サイズのチェック
            let array_size = if let Some((Token::BracketL, _)) = self.iter.peek() {
                self.iter.next(); // '[' を消費

                // サイズは定数（Number）のみ
                let size = match self.iter.next() {
                    Some((Token::Number(n), _)) => {
                        if *n <= 0 {
                            // エラー: 配列サイズは正の整数
                            let err_idx = self.add_parse_error(
                                &TokenInfo {
                                    code_pointer: start_pos,
                                },
                                "array size must be positive",
                            );
                            results.push(LocatedStatement {
                                statement: Statement::Invalid(err_idx),
                                location: SourceLocation::from_single(start_pos),
                            });
                            // エラー後はセミコロンまでスキップ
                            while let Some((token, _)) = self.iter.peek() {
                                if matches!(token, Token::Semicolon) {
                                    break;
                                }
                                self.iter.next();
                            }
                            self.iter.next(); // セミコロンを消費
                            return results;
                        }
                        *n
                    }
                    Some((_, token_info)) => {
                        let err_idx = self.add_parse_error(token_info, "expected array size");
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        while let Some((token, _)) = self.iter.peek() {
                            if matches!(token, Token::Semicolon) {
                                break;
                            }
                            self.iter.next();
                        }
                        self.iter.next();
                        return results;
                    }
                    None => {
                        let err_idx = self.add_end_error("unexpected end of input");
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        return results;
                    }
                };

                // ']' を消費
                match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
                Some(size)
            } else {
                None
            };

            // 初期化式のチェック
            if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
                // "(" を消費
                self.iter.next();

                if let Some(arr_size) = array_size {
                    // 配列の初期化: (val1, val2, val3, ...)
                    let mut init_values = Vec::new();

                    // 初期化値を読み取る
                    loop {
                        if let Some((Token::ParenthesisR, _)) = self.iter.peek() {
                            break;
                        }

                        let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
                        self.code_parse_error.append(&mut errs);
                        init_values.push(expr);

                        if let Some((Token::Comma, _)) = self.iter.peek() {
                            self.iter.next(); // カンマを消費
                        } else {
                            break;
                        }
                    }

                    // ")" を消費
                    match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

                    // サイズチェック
                    if init_values.len() > arr_size as usize {
                        let err_idx = self.add_parse_error(
                            &TokenInfo {
                                code_pointer: start_pos,
                            },
                            format!(
                                "too many initializers for array of size {}: got {}",
                                arr_size,
                                init_values.len()
                            ),
                        );
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        while let Some((token, _)) = self.iter.peek() {
                            if matches!(token, Token::Semicolon) {
                                break;
                            }
                            self.iter.next();
                        }
                        self.iter.next();
                        return results;
                    }

                    // 配列宣言を追加
                    let end_pos = self
                        .iter
                        .peek()
                        .map(|(_, info)| info.code_pointer)
                        .unwrap_or(start_pos);

                    results.push(LocatedStatement {
                        statement: Statement::VariableDeclaration(
                            id.clone(),
                            Box::new(Expression::Factor(0)),
                            is_static,
                            array_size,
                        ),
                        location: SourceLocation::new(start_pos, end_pos),
                    });

                    // 各要素への代入文を生成: arr[0] = val0, arr[1] = val1, ...
                    for (i, val_expr) in init_values.into_iter().enumerate() {
                        let assign_expr = Box::new(Expression::Operation2(
                            Operator2::Assign,
                            Box::new(Expression::ArrayAccess(
                                id.clone(),
                                Box::new(Expression::Factor(i as i64)),
                            )),
                            val_expr,
                        ));
                        results.push(LocatedStatement {
                            statement: Statement::Expression(assign_expr),
                            location: SourceLocation::new(start_pos, end_pos),
                        });
                    }
                } else {
                    // 通常変数の初期化: (expr)
                    let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
                    self.code_parse_error.append(&mut errs);

                    // ")" を消費
                    match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

                    // 代入式を構築: id = expr
                    let init_expr = Box::new(Expression::Operation2(
                        Operator2::Assign,
                        Box::new(Expression::Variable(id.clone())),
                        expr,
                    ));

                    let end_pos = self
                        .iter
                        .peek()
                        .map(|(_, info)| info.code_pointer)
                        .unwrap_or(start_pos);

                    results.push(LocatedStatement {
                        statement: Statement::VariableDeclaration(
                            id.clone(),
                            init_expr,
                            is_static,
                            None,
                        ),
                        location: SourceLocation::new(start_pos, end_pos),
                    });
                }
            } else {
                // 初期化式なし
                let end_pos = self
                    .iter
                    .peek()
                    .map(|(_, info)| info.code_pointer)
                    .unwrap_or(start_pos);

                results.push(LocatedStatement {
                    statement: Statement::VariableDeclaration(
                        id.clone(),
                        Box::new(Expression::Factor(0)),
                        is_static,
                        array_size,
                    ),
                    location: SourceLocation::new(start_pos, end_pos),
                });
            }

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
    }
}

pub(super) fn parse_to_statements(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> (Vec<LocatedStatement>, Vec<CodeParseError>) {
    StatementBuilder::parse(iter)
}

#[cfg(test)]
mod test;
