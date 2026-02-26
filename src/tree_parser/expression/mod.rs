use std::iter;

use crate::code_parse_error;

use crate::token_parser::{Keyword, TokenInfo};
use crate::tree_parser::statement::parse_to_statements;
use crate::{
    base::{CodeParseError, SourceLocation},
    token_parser::{PrettyToken, Token},
};

use super::{LocatedStatement, Statement};

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

#[derive(Clone, Debug)]
pub enum Operator2 {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Assign,
    PlusAssign,     // +=
    MinusAssign,    // -=
    MultiplyAssign, // *=
    DivideAssign,   // /=
    ModuloAssign,   // %=
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
}

#[derive(Clone, Debug)]
pub enum Operator1 {
    Negative,
    LogicalNot,
    Ref,   // &
    Deref, // *
}

/// 位置情報付きの Expression
#[derive(Clone, Debug)]
pub struct LocatedExpression {
    pub expression: Expression,
    pub location: SourceLocation,
}

#[derive(Clone, Debug)]
pub enum Expression {
    Operation1(Operator1, Box<LocatedExpression>),
    Operation2(Operator2, Box<LocatedExpression>, Box<LocatedExpression>),
    If(
        Box<LocatedExpression>,
        Vec<LocatedStatement>,
        Vec<LocatedStatement>,
    ),
    While(Box<LocatedExpression>, Vec<LocatedStatement>),
    Block(Vec<LocatedStatement>),                    // ブロックスコープ式
    Function(String, Vec<Box<LocatedExpression>>),   // 関数呼び出し
    Factor(i64),
    Variable(String),
    ArrayAccess(String, Box<LocatedExpression>), // 配列アクセス: arr[expr]
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
    ) -> (Box<LocatedExpression>, Vec<CodeParseError>) {
        let mut b = Self {
            iter,
            code_parse_error: vec![],
        };
        let e = b.parse_to_expression_tree_root();
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

    /// 現在のピーク位置を返す。トークンがなければ 0 を返す。
    fn current_pos(&mut self) -> usize {
        self.iter
            .peek()
            .map(|(_, info)| info.code_pointer)
            .unwrap_or(0)
    }

    /// Expression を LocatedExpression に包む
    fn located(&self, expr: Expression, start: usize, end: usize) -> Box<LocatedExpression> {
        Box::new(LocatedExpression {
            expression: expr,
            location: SourceLocation::new(start, end),
        })
    }

    fn parse_to_expression_tree_function_located(
        &mut self,
        name: &String,
        start: usize,
    ) -> Box<LocatedExpression> {
        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::ParenthesisL) {
            let end = self.current_pos();
            return self.located(Expression::Invalid(e), start, end);
        }

        let mut args = Vec::<Box<LocatedExpression>>::new();
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
                        self.add_parse_error(token_info, "unexpected comma");
                    }
                    self.iter.next();
                    let end = self.current_pos();
                    return self.located(Expression::Function(name.clone(), args), start, end);
                }
                Some((Token::Comma, token_info)) => {
                    if let State::Eval = state {
                        state = State::Comma;
                    } else {
                        // weak syntax error and proceed parsing
                        self.add_parse_error(token_info, "unexpected comma");
                    }
                    self.iter.next();
                }
                Some((_, token_info)) => {
                    if let State::Eval = state {
                        // weak syntax error and proceed parsing
                        self.add_parse_error(token_info, "missing comma");
                    }
                    let e = self.parse_to_expression_tree_root();
                    args.push(e);
                    state = State::Eval;
                }
                None => {
                    let e = self.add_end_error("unexpected end of input");
                    let end = self.current_pos();
                    return self.located(Expression::Invalid(e), start, end);
                }
            }
        }
    }

    fn parse_to_expression_tree_factor(&mut self) -> Box<LocatedExpression> {
        let start = self.current_pos();
        match self.iter.peek() {
            Some((Token::Number(val), _)) => {
                let val = *val;
                self.iter.next();
                let end = self.current_pos();
                self.located(Expression::Factor(val), start, end)
            }
            Some((Token::Identifier(id), _)) => {
                // TODO: confirm whether the identifier is reserved e.g. func
                let id = id.clone();
                self.iter.next();
                if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
                    return self.parse_to_expression_tree_function_located(&id, start);
                }
                // 配列アクセス: arr[expr]
                if let Some((Token::BracketL, _)) = self.iter.peek() {
                    self.iter.next(); // '[' を消費
                    let index_expr = self.parse_to_expression_tree_root();
                    match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
                    let end = self.current_pos();
                    return self.located(Expression::ArrayAccess(id, index_expr), start, end);
                }
                let end = self.current_pos();
                self.located(Expression::Variable(id), start, end)
            }
            Some((Token::ParenthesisL, _)) => {
                self.iter.next();
                let e = self.parse_to_expression_tree_root();

                if let Err(_) = match_expect_token!(self, self.iter.next(), Token::ParenthesisR) {
                    // weak syntax error and proceed parsing
                }
                // 括弧式はその中身の位置をそのまま引き継ぐ
                e
            }
            // if/while/block を factor レベルで解析
            Some((Token::Keyword(Keyword::If), _)) => {
                self.parse_to_expression_tree_if_impl()
            }
            Some((Token::Keyword(Keyword::While), _)) => {
                self.parse_to_expression_tree_while_impl()
            }
            Some((Token::BraceL, _)) => {
                self.parse_to_expression_tree_block_impl()
            }
            Some((_, token_info)) => {
                let e = self.add_parse_error(token_info, "unexpected token");
                let end = self.current_pos();
                self.located(Expression::Invalid(e), start, end)
            }
            _ => {
                let e = self.add_end_error("unexpected end of input");
                let end = self.current_pos();
                self.located(Expression::Invalid(e), start, end)
            }
        }
    }

    fn parse_to_expression_tree_unary(&mut self) -> Box<LocatedExpression> {
        let start = self.current_pos();
        let mut op_stack = vec![];
        loop {
            // `----` のような単行演算子が連続するものも許容する
            // よって `++x` のようなインクリメントは実装不可になる
            if let Some(token) = self.iter.peek() {
                match token {
                    (Token::Minus, _) => op_stack.push(Operator1::Negative),
                    (Token::Exclamation, _) => op_stack.push(Operator1::LogicalNot),
                    (Token::Ampersand, _) => op_stack.push(Operator1::Ref),
                    (Token::Asterisk, _) => op_stack.push(Operator1::Deref),
                    _ => break,
                }
            } else {
                break;
            };
            self.iter.next();
        }
        let mut left = self.parse_to_expression_tree_factor();
        while let Some(op) = op_stack.pop() {
            let end = left.location.end;
            left = self.located(Expression::Operation1(op, left), start, end);
        }
        left
    }

    fn parse_to_expression_tree_mul(&mut self) -> Box<LocatedExpression> {
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
            let start = left.location.start;
            let end = right.location.end;
            left = self.located(Expression::Operation2(op, left, right), start, end);
        }
    }

    fn parse_to_expression_tree_plus(&mut self) -> Box<LocatedExpression> {
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
            let start = left.location.start;
            let end = right.location.end;
            left = self.located(Expression::Operation2(op, left, right), start, end);
        }
    }

    fn parse_to_expression_tree_compare(&mut self) -> Box<LocatedExpression> {
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
            let start = left.location.start;
            let end = right.location.end;
            left = self.located(Expression::Operation2(op, left, right), start, end);
        }
    }

    fn parse_to_expression_tree_logical_and(&mut self) -> Box<LocatedExpression> {
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
            let start = left.location.start;
            let end = right.location.end;
            left = self.located(Expression::Operation2(op, left, right), start, end);
        }
    }

    fn parse_to_expression_tree_logical_or(&mut self) -> Box<LocatedExpression> {
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
            let start = left.location.start;
            let end = right.location.end;
            left = self.located(Expression::Operation2(op, left, right), start, end);
        }
    }

    fn parse_to_expression_tree_assign(&mut self) -> Box<LocatedExpression> {
        let left = self.parse_to_expression_tree_logical_or();
        let op = if let Some(token) = self.iter.peek() {
            match token {
                (Token::SingleEqual, _) => Operator2::Assign,
                (Token::PlusEqual, _) => Operator2::PlusAssign,
                (Token::MinusEqual, _) => Operator2::MinusAssign,
                (Token::AsteriskEqual, _) => Operator2::MultiplyAssign,
                (Token::SlashEqual, _) => Operator2::DivideAssign,
                (Token::PercentEqual, _) => Operator2::ModuloAssign,
                _ => return left,
            }
        } else {
            return left;
        };
        self.iter.next();
        // 右辺で代入を再帰的に許可（右結合）
        let right = self.parse_to_expression_tree_assign();
        let start = left.location.start;
        let end = right.location.end;
        self.located(Expression::Operation2(op, left, right), start, end)
    }

    // while 式の実際の解析処理
    fn parse_to_expression_tree_while_impl(&mut self) -> Box<LocatedExpression> {
        let start = self.current_pos();
        let token = self.iter.next(); // while キーワードを消費
        assert!(matches!(token, Some((Token::Keyword(Keyword::While), _))));

        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
            let end = self.current_pos();
            return self.located(Expression::Invalid(e), start, end);
        }
        let cond = self.parse_to_expression_tree_root();
        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::BraceL) {
            let end = self.current_pos();
            return self.located(Expression::Invalid(e), start, end);
        }
        let (stat, mut stat_err) = parse_to_statements(self.iter);
        if !stat_err.is_empty() {
            self.code_parse_error.append(&mut stat_err);
        }
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        let end = self.current_pos();
        self.located(Expression::While(cond, stat), start, end)
    }

    // ブロックスコープ式の解析処理
    fn parse_to_expression_tree_block_impl(&mut self) -> Box<LocatedExpression> {
        let start = self.current_pos();
        self.iter.next(); // '{' を消費
        let (stat, mut stat_err) = parse_to_statements(self.iter);
        if !stat_err.is_empty() {
            self.code_parse_error.append(&mut stat_err);
        }
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        let end = self.current_pos();
        self.located(Expression::Block(stat), start, end)
    }

    // if 式の実際の解析処理
    fn parse_to_expression_tree_if_impl(&mut self) -> Box<LocatedExpression> {
        let start = self.current_pos();
        let token = self.iter.next(); // if キーワードを消費
        assert!(matches!(token, Some((Token::Keyword(Keyword::If), _))));

        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::Colon) {
            let end = self.current_pos();
            return self.located(Expression::Invalid(e), start, end);
        }
        let cond = self.parse_to_expression_tree_root();
        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::BraceL) {
            // NOTE: statements ではなく expression が来ても許容、でいいかもね?
            let end = self.current_pos();
            return self.located(Expression::Invalid(e), start, end);
        }

        let (stats_true, mut stats_err) = parse_to_statements(self.iter);
        if !stats_err.is_empty() {
            self.code_parse_error.append(&mut stats_err);
        }

        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);

        let stats_false = match self.iter.peek() {
            Some((Token::Keyword(Keyword::Else), token_info)) => {
                let else_start = token_info.code_pointer;
                self.iter.next();
                match_expect_token_unused!(self, self.iter.next(), Token::Colon);

                match self.iter.peek() {
                    Some((Token::Keyword(Keyword::If), _)) => {
                        // else: if: cond {}
                        let if_expr = self.parse_to_expression_tree_if_impl();
                        let end_pos = self
                            .iter
                            .peek()
                            .map(|(_, info)| info.code_pointer)
                            .unwrap_or(else_start);
                        vec![LocatedStatement {
                            statement: Statement::Expression(if_expr),
                            location: SourceLocation::new(else_start, end_pos),
                        }]
                    }
                    _ => {
                        match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
                        let (stats, mut stats_err) = parse_to_statements(self.iter);
                        if !stats_err.is_empty() {
                            self.code_parse_error.append(&mut stats_err);
                        }
                        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
                        stats
                    }
                }
            }
            _ => {
                vec![]
            }
        };
        let end = self.current_pos();
        self.located(Expression::If(cond, stats_true, stats_false), start, end)
    }

    fn parse_to_expression_tree_root(&mut self) -> Box<LocatedExpression> {
        // if/while は factor レベルで解析されるため、ここでは assign から開始
        self.parse_to_expression_tree_assign()
    }
}

pub(super) fn parse_to_expression_tree_root(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> (Box<LocatedExpression>, Vec<CodeParseError>) {
    ExpressionBuilder::parse(iter)
}

#[cfg(test)]
mod test;
