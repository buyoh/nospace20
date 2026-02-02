use std::iter;

use crate::{
    base::CodeParseError,
    code_parse_error,
    token_parser::{Keyword, PrettyToken, Token, TokenInfo},
};

use super::expression::*;

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

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
    code_parse_error: Vec<CodeParseError>,
}

impl<'b: 'a, 'a> StatementBuilder<'b, 'a> {
    fn parse(
        iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    ) -> (Vec<Statement>, Vec<CodeParseError>) {
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
            .push(code_parse_error!(msg.to_string()));
        i
    }

    fn parse_to_statements_block(&mut self) -> Vec<Statement> {
        match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
        let ss = self.parse_to_statements();
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        return ss;
    }

    fn parse_to_statements_let(&mut self) -> Statement {
        if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Let)) {
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
        if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Func)) {
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
        if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Keyword(Keyword::Return))
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
                (Token::Keyword(Keyword::Let), _) => {
                    statements.push(self.parse_to_statements_let());
                    continue;
                }
                (Token::Keyword(Keyword::Func), _) => {
                    statements.push(self.parse_to_statements_func());
                    continue;
                }
                (Token::Keyword(Keyword::Return), _) => {
                    statements.push(self.parse_to_statements_return());
                    continue;
                }
                (Token::Keyword(Keyword::Break), _) => {
                    self.iter.next();
                    statements.push(Statement::Break);
                    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                    continue;
                }
                (Token::Keyword(Keyword::Continue), _) => {
                    self.iter.next();
                    statements.push(Statement::Continue);
                    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                    continue;
                }
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
) -> (Vec<Statement>, Vec<CodeParseError>) {
    StatementBuilder::parse(iter)
}

#[cfg(test)]
mod tests {
    use super::*;

    // テストヘルパー: トークン生成関数
    fn token_keyword_let() -> PrettyToken {
        (Token::Keyword(Keyword::Let), TokenInfo { code_pointer: 0 })
    }

    fn token_keyword_func() -> PrettyToken {
        (Token::Keyword(Keyword::Func), TokenInfo { code_pointer: 0 })
    }

    fn token_keyword_return() -> PrettyToken {
        (
            Token::Keyword(Keyword::Return),
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_keyword_break() -> PrettyToken {
        (
            Token::Keyword(Keyword::Break),
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_keyword_continue() -> PrettyToken {
        (
            Token::Keyword(Keyword::Continue),
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_ident(name: &str) -> PrettyToken {
        (
            Token::Identifier(name.to_string()),
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_number(value: i64) -> PrettyToken {
        (Token::Number(value), TokenInfo { code_pointer: 0 })
    }

    fn token_colon() -> PrettyToken {
        (Token::Colon, TokenInfo { code_pointer: 0 })
    }

    fn token_semicolon() -> PrettyToken {
        (Token::Semicolon, TokenInfo { code_pointer: 0 })
    }

    fn token_paren_l() -> PrettyToken {
        (Token::ParenthesisL, TokenInfo { code_pointer: 0 })
    }

    fn token_paren_r() -> PrettyToken {
        (Token::ParenthesisR, TokenInfo { code_pointer: 0 })
    }

    fn token_brace_l() -> PrettyToken {
        (Token::BraceL, TokenInfo { code_pointer: 0 })
    }

    fn token_brace_r() -> PrettyToken {
        (Token::BraceR, TokenInfo { code_pointer: 0 })
    }

    fn token_comma() -> PrettyToken {
        (Token::Comma, TokenInfo { code_pointer: 0 })
    }

    fn token_op_single_equal() -> PrettyToken {
        (Token::SingleEqual, TokenInfo { code_pointer: 0 })
    }

    // ヘルパー: パース実行
    fn parse_stmts(tokens: Vec<PrettyToken>) -> (Vec<Statement>, Vec<CodeParseError>) {
        parse_to_statements(&mut tokens.iter().peekable())
    }

    #[test]
    fn test_parse_let_statement() {
        // let: x;
        let tokens = vec![
            token_keyword_let(),
            token_colon(),
            token_ident("x"),
            token_semicolon(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::VariableDeclaration(name, expr) => {
                assert_eq!(name, "x");
                match **expr {
                    Expression::Factor(0) => (), // デフォルト値は0
                    _ => panic!("Expected Factor(0)"),
                }
            }
            _ => panic!("Expected Statement::VariableDeclaration"),
        }
    }

    #[test]
    fn test_parse_break_statement() {
        // break;
        let tokens = vec![token_keyword_break(), token_semicolon()];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::Break => (),
            _ => panic!("Expected Statement::Break"),
        }
    }

    #[test]
    fn test_parse_continue_statement() {
        // continue;
        let tokens = vec![token_keyword_continue(), token_semicolon()];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::Continue => (),
            _ => panic!("Expected Statement::Continue"),
        }
    }

    #[test]
    fn test_parse_return_statement() {
        // return: 42;
        let tokens = vec![
            token_keyword_return(),
            token_colon(),
            token_number(42),
            token_semicolon(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::Return(expr) => match **expr {
                Expression::Factor(42) => (),
                _ => panic!("Expected Factor(42)"),
            },
            _ => panic!("Expected Statement::Return"),
        }
    }

    #[test]
    fn test_parse_expression_statement() {
        // x = 10;
        let tokens = vec![
            token_ident("x"),
            token_op_single_equal(),
            token_number(10),
            token_semicolon(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::Expression(expr) => match **expr {
                Expression::Operation2(Operator2::Assign, _, _) => (),
                _ => panic!("Expected Operation2(Assign)"),
            },
            _ => panic!("Expected Statement::Expression"),
        }
    }

    #[test]
    fn test_parse_func_no_args() {
        // func: foo() {}
        let tokens = vec![
            token_keyword_func(),
            token_colon(),
            token_ident("foo"),
            token_paren_l(),
            token_paren_r(),
            token_brace_l(),
            token_brace_r(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::FunctionDeclaration(name, args, body) => {
                assert_eq!(name, "foo");
                assert_eq!(args.len(), 0);
                assert_eq!(body.len(), 0);
            }
            _ => panic!("Expected Statement::FunctionDeclaration"),
        }
    }

    #[test]
    fn test_parse_func_one_arg() {
        // func: bar(x) {}
        let tokens = vec![
            token_keyword_func(),
            token_colon(),
            token_ident("bar"),
            token_paren_l(),
            token_ident("x"),
            token_paren_r(),
            token_brace_l(),
            token_brace_r(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::FunctionDeclaration(name, args, body) => {
                assert_eq!(name, "bar");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], "x");
                assert_eq!(body.len(), 0);
            }
            _ => panic!("Expected Statement::FunctionDeclaration"),
        }
    }

    #[test]
    fn test_parse_func_multi_args() {
        // func: baz(x, y) {}
        let tokens = vec![
            token_keyword_func(),
            token_colon(),
            token_ident("baz"),
            token_paren_l(),
            token_ident("x"),
            token_comma(),
            token_ident("y"),
            token_paren_r(),
            token_brace_l(),
            token_brace_r(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::FunctionDeclaration(name, args, body) => {
                assert_eq!(name, "baz");
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], "x");
                assert_eq!(args[1], "y");
                assert_eq!(body.len(), 0);
            }
            _ => panic!("Expected Statement::FunctionDeclaration"),
        }
    }

    #[test]
    fn test_parse_func_with_body() {
        // func: foo() { return: 42; }
        let tokens = vec![
            token_keyword_func(),
            token_colon(),
            token_ident("foo"),
            token_paren_l(),
            token_paren_r(),
            token_brace_l(),
            token_keyword_return(),
            token_colon(),
            token_number(42),
            token_semicolon(),
            token_brace_r(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::FunctionDeclaration(name, args, body) => {
                assert_eq!(name, "foo");
                assert_eq!(args.len(), 0);
                assert_eq!(body.len(), 1);
                match &body[0] {
                    Statement::Return(_) => (),
                    _ => panic!("Expected Statement::Return in body"),
                }
            }
            _ => panic!("Expected Statement::FunctionDeclaration"),
        }
    }

    #[test]
    fn test_parse_multiple_statements() {
        // let: x;
        // let: y;
        let tokens = vec![
            token_keyword_let(),
            token_colon(),
            token_ident("x"),
            token_semicolon(),
            token_keyword_let(),
            token_colon(),
            token_ident("y"),
            token_semicolon(),
        ];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 2);
        match &stmts[0] {
            Statement::VariableDeclaration(name, _) => {
                assert_eq!(name, "x");
            }
            _ => panic!("Expected Statement::VariableDeclaration"),
        }
        match &stmts[1] {
            Statement::VariableDeclaration(name, _) => {
                assert_eq!(name, "y");
            }
            _ => panic!("Expected Statement::VariableDeclaration"),
        }
    }

    #[test]
    fn test_parse_empty_statements() {
        let tokens = vec![];
        let (stmts, errs) = parse_stmts(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        assert_eq!(stmts.len(), 0);
    }
}
