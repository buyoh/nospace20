use std::iter;

use crate::code_parse_error;

use crate::token_parser::{Keyword, TokenInfo};
use crate::tree_parser::statement::parse_to_statements;
use crate::{
    base::CodeParseError,
    token_parser::{PrettyToken, Token},
};

use super::Statement;

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

#[derive(Clone)] // TODO: REMOVE
pub enum Operator2 {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Assign,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
}

#[derive(Clone)] // TODO: REMOVE
pub enum Operator1 {
    Negative,
    LogicalNot,
}

#[derive(Clone)] // TODO: REMOVE
pub enum Expression {
    Operation1(Operator1, Box<Expression>),
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
    code_parse_error: Vec<CodeParseError>,
}

impl<'b: 'a, 'a> ExpressionBuilder<'b, 'a> {
    fn parse(
        iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    ) -> (Box<Expression>, Vec<CodeParseError>) {
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
            .push(code_parse_error!(msg.to_string()));
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

    fn parse_to_expression_tree_unary(&mut self) -> Box<Expression> {
        let mut op_stack = vec![];
        loop {
            // `----` のような単行演算子が連続するものも許容する
            // よって `++x` のようなインクリメントは実装不可になる
            if let Some(token) = self.iter.peek() {
                match token {
                    (Token::Minus, _) => op_stack.push(Operator1::Negative),
                    (Token::Exclamation, _) => op_stack.push(Operator1::LogicalNot),
                    _ => break,
                }
            } else {
                break;
            };
            self.iter.next();
        }
        let mut left = self.parse_to_expression_tree_factor();
        while let Some(op) = op_stack.pop() {
            left = Box::new(Expression::Operation1(op, left))
        }
        left
    }

    fn parse_to_expression_tree_mul(&mut self) -> Box<Expression> {
        let mut left = self.parse_to_expression_tree_unary();
        loop {
            let op = if let Some(token) = self.iter.peek() {
                match token {
                    (Token::Asterisk, _) => Operator2::Multiply,
                    (Token::Slash, _) => Operator2::Divide,
                    (Token::Percent, _) => Operator2::Modulo,
                    _ => return left,
                }
            } else {
                return left;
            };
            self.iter.next();
            let right = self.parse_to_expression_tree_unary();
            left = Box::new(Expression::Operation2(op, left, right))
        }
    }

    fn parse_to_expression_tree_plus(&mut self) -> Box<Expression> {
        let mut left = self.parse_to_expression_tree_mul();
        loop {
            let op = if let Some(token) = self.iter.peek() {
                match token {
                    (Token::Plus, _) => Operator2::Plus,
                    (Token::Minus, _) => Operator2::Minus,
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

    fn parse_to_expression_tree_compare(&mut self) -> Box<Expression> {
        let mut left = self.parse_to_expression_tree_plus();
        loop {
            let op = if let Some(token) = self.iter.peek() {
                match token {
                    (Token::DoubleEqual, _) => Operator2::Equal,
                    (Token::NotEqual, _) => Operator2::NotEqual,
                    (Token::Less, _) => Operator2::Less,
                    (Token::LessEqual, _) => Operator2::LessEqual,
                    (Token::Greater, _) => Operator2::Greater,
                    (Token::GreaterEqual, _) => Operator2::GreaterEqual,
                    _ => return left,
                }
            } else {
                return left;
            };
            self.iter.next();
            let right = self.parse_to_expression_tree_plus();
            left = Box::new(Expression::Operation2(op, left, right));
        }
    }

    fn parse_to_expression_tree_logical_and(&mut self) -> Box<Expression> {
        let mut left = self.parse_to_expression_tree_compare();
        loop {
            let op = if let Some(token) = self.iter.peek() {
                match token {
                    (Token::DoubleAmpersand, _) => Operator2::LogicalAnd,
                    _ => return left,
                }
            } else {
                return left;
            };
            self.iter.next();
            let right = self.parse_to_expression_tree_compare();
            left = Box::new(Expression::Operation2(op, left, right));
        }
    }

    fn parse_to_expression_tree_logical_or(&mut self) -> Box<Expression> {
        let mut left = self.parse_to_expression_tree_logical_and();
        loop {
            let op = if let Some(token) = self.iter.peek() {
                match token {
                    (Token::DoublePipe, _) => Operator2::LogicalOr,
                    _ => return left,
                }
            } else {
                return left;
            };
            self.iter.next();
            let right = self.parse_to_expression_tree_logical_and();
            left = Box::new(Expression::Operation2(op, left, right));
        }
    }

    fn parse_to_expression_tree_assign(&mut self) -> Box<Expression> {
        let left = self.parse_to_expression_tree_logical_or();
        let op = if let Some(token) = self.iter.peek() {
            match token {
                (Token::SingleEqual, _) => Operator2::Assign,
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
) -> (Box<Expression>, Vec<CodeParseError>) {
    ExpressionBuilder::parse(iter)
}

#[cfg(test)]
mod tests {
    use super::*;

    // テストヘルパー: トークン生成関数
    fn token_number(value: i64) -> PrettyToken {
        (
            Token::Number(value),
            TokenInfo {
                code_pointer: 0,
            },
        )
    }

    fn token_ident(name: &str) -> PrettyToken {
        (
            Token::Identifier(name.to_string()),
            TokenInfo {
                code_pointer: 0,
            },
        )
    }

    fn token_op_plus() -> PrettyToken {
        (Token::Plus, TokenInfo { code_pointer: 0 })
    }

    fn token_op_minus() -> PrettyToken {
        (Token::Minus, TokenInfo { code_pointer: 0 })
    }

    fn token_op_asterisk() -> PrettyToken {
        (Token::Asterisk, TokenInfo { code_pointer: 0 })
    }

    fn token_op_slash() -> PrettyToken {
        (Token::Slash, TokenInfo { code_pointer: 0 })
    }

    fn token_op_percent() -> PrettyToken {
        (Token::Percent, TokenInfo { code_pointer: 0 })
    }

    fn token_op_double_equal() -> PrettyToken {
        (
            Token::DoubleEqual,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_not_equal() -> PrettyToken {
        (
            Token::NotEqual,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_less() -> PrettyToken {
        (Token::Less, TokenInfo { code_pointer: 0 })
    }

    fn token_op_less_equal() -> PrettyToken {
        (
            Token::LessEqual,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_greater() -> PrettyToken {
        (
            Token::Greater,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_greater_equal() -> PrettyToken {
        (
            Token::GreaterEqual,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_logical_and() -> PrettyToken {
        (
            Token::DoubleAmpersand,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_logical_or() -> PrettyToken {
        (
            Token::DoublePipe,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_single_equal() -> PrettyToken {
        (
            Token::SingleEqual,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_op_exclamation() -> PrettyToken {
        (
            Token::Exclamation,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_paren_l() -> PrettyToken {
        (
            Token::ParenthesisL,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_paren_r() -> PrettyToken {
        (
            Token::ParenthesisR,
            TokenInfo { code_pointer: 0 },
        )
    }

    fn token_comma() -> PrettyToken {
        (Token::Comma, TokenInfo { code_pointer: 0 })
    }

    // ヘルパー: パース実行
    fn parse_expr(tokens: Vec<PrettyToken>) -> (Box<Expression>, Vec<CodeParseError>) {
        parse_to_expression_tree_root(&mut tokens.iter().peekable())
    }

    #[test]
    fn test_parse_literal_number() {
        let tokens = vec![token_number(42)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Factor(val) => assert_eq!(val, 42),
            _ => panic!("Expected Expression::Factor"),
        }
    }

    #[test]
    fn test_parse_variable() {
        let tokens = vec![token_ident("foo")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Variable(name) => assert_eq!(name, "foo"),
            _ => panic!("Expected Expression::Variable"),
        }
    }

    #[test]
    fn test_parse_add() {
        let tokens = vec![token_number(1), token_op_plus(), token_number(2)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Plus, left, right) => {
                match (*left, *right) {
                    (Expression::Factor(1), Expression::Factor(2)) => (),
                    _ => panic!("Expected Factor(1) + Factor(2)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Plus)"),
        }
    }

    #[test]
    fn test_parse_subtract() {
        let tokens = vec![token_number(5), token_op_minus(), token_number(3)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Minus, left, right) => {
                match (*left, *right) {
                    (Expression::Factor(5), Expression::Factor(3)) => (),
                    _ => panic!("Expected Factor(5) - Factor(3)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Minus)"),
        }
    }

    #[test]
    fn test_parse_multiply() {
        let tokens = vec![token_number(3), token_op_asterisk(), token_number(4)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Multiply, left, right) => {
                match (*left, *right) {
                    (Expression::Factor(3), Expression::Factor(4)) => (),
                    _ => panic!("Expected Factor(3) * Factor(4)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Multiply)"),
        }
    }

    #[test]
    fn test_parse_divide() {
        let tokens = vec![token_number(10), token_op_slash(), token_number(2)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Divide, left, right) => {
                match (*left, *right) {
                    (Expression::Factor(10), Expression::Factor(2)) => (),
                    _ => panic!("Expected Factor(10) / Factor(2)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Divide)"),
        }
    }

    #[test]
    fn test_parse_modulo() {
        let tokens = vec![token_number(10), token_op_percent(), token_number(3)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Modulo, left, right) => {
                match (*left, *right) {
                    (Expression::Factor(10), Expression::Factor(3)) => (),
                    _ => panic!("Expected Factor(10) % Factor(3)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Modulo)"),
        }
    }

    #[test]
    fn test_parse_precedence_mul_before_add() {
        // 1 + 2 * 3 => 1 + (2 * 3)
        let tokens = vec![
            token_number(1),
            token_op_plus(),
            token_number(2),
            token_op_asterisk(),
            token_number(3),
        ];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Plus, left, right) => {
                match (*left, *right) {
                    (Expression::Factor(1), Expression::Operation2(Operator2::Multiply, l2, r2)) => {
                        match (*l2, *r2) {
                            (Expression::Factor(2), Expression::Factor(3)) => (),
                            _ => panic!("Expected Factor(2) * Factor(3)"),
                        }
                    }
                    _ => panic!("Expected 1 + (2 * 3)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Plus)"),
        }
    }

    #[test]
    fn test_parse_parenthesis() {
        // (1 + 2) * 3 => (1 + 2) * 3
        let tokens = vec![
            token_paren_l(),
            token_number(1),
            token_op_plus(),
            token_number(2),
            token_paren_r(),
            token_op_asterisk(),
            token_number(3),
        ];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Multiply, left, right) => {
                match (*left, *right) {
                    (Expression::Operation2(Operator2::Plus, l2, r2), Expression::Factor(3)) => {
                        match (*l2, *r2) {
                            (Expression::Factor(1), Expression::Factor(2)) => (),
                            _ => panic!("Expected Factor(1) + Factor(2)"),
                        }
                    }
                    _ => panic!("Expected (1 + 2) * 3"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Multiply)"),
        }
    }

    #[test]
    fn test_parse_function_call_no_args() {
        // foo()
        let tokens = vec![token_ident("foo"), token_paren_l(), token_paren_r()];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Function(name, args) => {
                assert_eq!(name, "foo");
                assert_eq!(args.len(), 0);
            }
            _ => panic!("Expected Expression::Function"),
        }
    }

    #[test]
    fn test_parse_function_call_one_arg() {
        // foo(42)
        let tokens = vec![
            token_ident("foo"),
            token_paren_l(),
            token_number(42),
            token_paren_r(),
        ];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Function(name, args) => {
                assert_eq!(name, "foo");
                assert_eq!(args.len(), 1);
                match *args[0] {
                    Expression::Factor(42) => (),
                    _ => panic!("Expected Factor(42)"),
                }
            }
            _ => panic!("Expected Expression::Function"),
        }
    }

    #[test]
    fn test_parse_function_call_multi_args() {
        // foo(1, 2)
        let tokens = vec![
            token_ident("foo"),
            token_paren_l(),
            token_number(1),
            token_comma(),
            token_number(2),
            token_paren_r(),
        ];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Function(name, args) => {
                assert_eq!(name, "foo");
                assert_eq!(args.len(), 2);
                match (*args[0].clone(), *args[1].clone()) {
                    (Expression::Factor(1), Expression::Factor(2)) => (),
                    _ => panic!("Expected Factor(1), Factor(2)"),
                }
            }
            _ => panic!("Expected Expression::Function"),
        }
    }

    #[test]
    fn test_parse_unary_minus() {
        // -1
        let tokens = vec![token_op_minus(), token_number(1)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation1(Operator1::Negative, inner) => match *inner {
                Expression::Factor(1) => (),
                _ => panic!("Expected Factor(1)"),
            },
            _ => panic!("Expected Expression::Operation1(Negative)"),
        }
    }

    #[test]
    fn test_parse_unary_logical_not() {
        // !true (represented as !1)
        let tokens = vec![token_op_exclamation(), token_number(1)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation1(Operator1::LogicalNot, inner) => match *inner {
                Expression::Factor(1) => (),
                _ => panic!("Expected Factor(1)"),
            },
            _ => panic!("Expected Expression::Operation1(LogicalNot)"),
        }
    }

    #[test]
    fn test_parse_comparison_equal() {
        // a == b
        let tokens = vec![token_ident("a"), token_op_double_equal(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Equal, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) == Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Equal)"),
        }
    }

    #[test]
    fn test_parse_comparison_not_equal() {
        // a != b
        let tokens = vec![token_ident("a"), token_op_not_equal(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::NotEqual, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) != Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(NotEqual)"),
        }
    }

    #[test]
    fn test_parse_comparison_less() {
        // a < b
        let tokens = vec![token_ident("a"), token_op_less(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Less, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) < Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Less)"),
        }
    }

    #[test]
    fn test_parse_comparison_less_equal() {
        // a <= b
        let tokens = vec![token_ident("a"), token_op_less_equal(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::LessEqual, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) <= Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(LessEqual)"),
        }
    }

    #[test]
    fn test_parse_comparison_greater() {
        // a > b
        let tokens = vec![token_ident("a"), token_op_greater(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Greater, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) > Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Greater)"),
        }
    }

    #[test]
    fn test_parse_comparison_greater_equal() {
        // a >= b
        let tokens = vec![token_ident("a"), token_op_greater_equal(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::GreaterEqual, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) >= Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(GreaterEqual)"),
        }
    }

    #[test]
    fn test_parse_logical_and() {
        // a && b
        let tokens = vec![token_ident("a"), token_op_logical_and(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::LogicalAnd, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) && Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(LogicalAnd)"),
        }
    }

    #[test]
    fn test_parse_logical_or() {
        // a || b
        let tokens = vec![token_ident("a"), token_op_logical_or(), token_ident("b")];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::LogicalOr, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Variable(b)) => {
                        assert_eq!(a, "a");
                        assert_eq!(b, "b");
                    }
                    _ => panic!("Expected Variable(a) || Variable(b)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(LogicalOr)"),
        }
    }

    #[test]
    fn test_parse_assignment() {
        // a = 10
        let tokens = vec![token_ident("a"), token_op_single_equal(), token_number(10)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation2(Operator2::Assign, left, right) => {
                match (*left, *right) {
                    (Expression::Variable(a), Expression::Factor(10)) => {
                        assert_eq!(a, "a");
                    }
                    _ => panic!("Expected Variable(a) = Factor(10)"),
                }
            }
            _ => panic!("Expected Expression::Operation2(Assign)"),
        }
    }

    #[test]
    fn test_parse_error_unclosed_paren() {
        // (1 + 2  (missing close paren)
        let tokens = vec![
            token_paren_l(),
            token_number(1),
            token_op_plus(),
            token_number(2),
        ];
        let (expr, errs) = parse_expr(tokens);
        assert!(!errs.is_empty(), "Expected errors for unclosed paren");
        // エラーが発生することを確認
        match *expr {
            Expression::Operation2(Operator2::Plus, _, _) => (), // パースは進むがエラーも記録される
            _ => {}
        }
    }

    #[test]
    fn test_parse_double_unary_minus() {
        // --5 => -(-5)
        let tokens = vec![token_op_minus(), token_op_minus(), token_number(5)];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        match *expr {
            Expression::Operation1(Operator1::Negative, inner1) => match *inner1 {
                Expression::Operation1(Operator1::Negative, inner2) => match *inner2 {
                    Expression::Factor(5) => (),
                    _ => panic!("Expected Factor(5)"),
                },
                _ => panic!("Expected Operation1(Negative)"),
            },
            _ => panic!("Expected Expression::Operation1(Negative)"),
        }
    }

    #[test]
    fn test_parse_complex_precedence() {
        // 1 + 2 * 3 < 10 && 5 == 5
        let tokens = vec![
            token_number(1),
            token_op_plus(),
            token_number(2),
            token_op_asterisk(),
            token_number(3),
            token_op_less(),
            token_number(10),
            token_op_logical_and(),
            token_number(5),
            token_op_double_equal(),
            token_number(5),
        ];
        let (expr, errs) = parse_expr(tokens);
        assert!(errs.is_empty(), "Expected no errors");
        // 複雑な優先順位が正しく処理されることを確認
        match *expr {
            Expression::Operation2(Operator2::LogicalAnd, _, _) => (),
            _ => panic!("Expected top-level LogicalAnd"),
        }
    }
}
