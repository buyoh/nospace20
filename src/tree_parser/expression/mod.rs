use std::iter;

use crate::code_parse_error;

use crate::token_parser::{Keyword, TokenInfo};
use crate::tree_parser::statement::parse_to_statements_at;
use crate::tree_parser::{MAX_NESTING_DEPTH, NESTING_TOO_DEEP_MESSAGE};
use crate::{
    base::{CodeParseError, SourceLocation},
    token_parser::{PrettyToken, Token},
};

use super::{LocatedStatement, Statement};

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

/// 二項演算子
#[derive(Clone, Debug, PartialEq)]
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

/// 単項演算子
#[derive(Clone, Debug)]
pub enum Operator1 {
    Negative,
    LogicalNot,
    Ref,   // &
    Deref, // *
}

/// 型指定（型注釈に使用）
#[derive(Clone, Debug)]
pub enum TypeSpec {
    Int,
    Void,
    Named(String),
    Array(Box<TypeSpec>, usize),
    Ref(Box<TypeSpec>),
}

/// 位置情報付きの Expression
#[derive(Clone, Debug)]
pub struct LocatedExpression {
    pub expression: Expression,
    pub location: SourceLocation,
}

/// 式の AST ノード。位置は `LocatedExpression` が持つ
#[derive(Clone, Debug)]
pub enum Expression {
    Operation1(Operator1, Box<LocatedExpression>),
    Operation2(Operator2, Box<LocatedExpression>, Box<LocatedExpression>),
    If(
        Box<LocatedExpression>,
        Vec<LocatedStatement>,
        Vec<LocatedStatement>,
    ),
    Block(Vec<LocatedStatement>),                  // ブロックスコープ式
    Function(String, Vec<Box<LocatedExpression>>), // 関数呼び出し
    /// 型注釈: expr @ type_spec
    TypeAnnotation(Box<LocatedExpression>, TypeSpec),
    /// フィールドアクセス: expr.field_name
    FieldAccess(Box<LocatedExpression>, String),
    /// フィールド配列アクセス: expr.field_name[index]
    FieldArrayAccess(Box<LocatedExpression>, String, Box<LocatedExpression>),
    /// 構造体リテラル: struct: Name(expr, expr, ...)
    StructLiteral(String, Vec<LocatedExpression>),
    Factor(i64),
    Variable(String),
    ArrayAccess(String, Box<LocatedExpression>), // 配列アクセス: arr[expr]
    Invalid(usize), // NOTE: CodeParseError に関連する情報を入れる。今は CodeParseError の
                    // インデックスを利用。 本来は ExpressionBuilder 単位ではなく、全体で独立した
                    // インデックスを利用するべき。
                    // 構文木のノードからエラー情報を参照したい目的は特に無いので、使われていない。
}

//

/// 式の構文解析器
struct ExpressionBuilder<'b: 'a, 'a> {
    iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
    code_parse_error: Vec<CodeParseError>,
    /// 現在のネスト深度（文パーサとの相互呼び出しでも引き継ぐ。`MAX_NESTING_DEPTH` 参照）
    depth: usize,
    /// 解析中に到達したネスト深度の最大値（部分木の高さの計測に使う）
    peak: usize,
}

impl<'b: 'a, 'a> ExpressionBuilder<'b, 'a> {
    /// 戻り値の 3 番目は解析中に到達したネスト深度の最大値
    fn parse(
        iter: &'a mut iter::Peekable<std::slice::Iter<'b, PrettyToken>>,
        depth: usize,
    ) -> (Box<LocatedExpression>, Vec<CodeParseError>, usize) {
        let mut b = Self {
            iter,
            code_parse_error: vec![],
            depth,
            peak: depth,
        };
        let e = b.parse_to_expression_tree_root();
        (e, b.code_parse_error, b.peak)
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

    /// ネストを `levels` 段深くして `f` を実行する
    ///
    /// 上限（`MAX_NESTING_DEPTH`）を超える場合は `f` を実行せず（トークンも消費せず）、
    /// エラーを記録してそのインデックスを `Err` で返す。
    // バグ修正: 従来はネスト深度に上限がなく、深い括弧・単項演算子・長い演算子の連鎖などで
    // パーサや後段（意味解析・コンパイラ）の再帰が Rust のスタックを使い切って abort していた。
    // 解決策: 構文木のネスト深度を数え、上限を超える入力はパースエラーにする。
    fn nested<T>(&mut self, levels: usize, f: impl FnOnce(&mut Self) -> T) -> Result<T, usize> {
        if self.depth + levels > MAX_NESTING_DEPTH {
            return Err(self.add_nesting_error());
        }
        self.depth += levels;
        self.peak = self.peak.max(self.depth);
        let r = f(self);
        self.depth -= levels;
        Ok(r)
    }

    fn add_nesting_error(&mut self) -> usize {
        match self.iter.peek() {
            Some((_, token_info)) => self.add_parse_error(token_info, NESTING_TOO_DEEP_MESSAGE),
            None => self.add_end_error(NESTING_TOO_DEEP_MESSAGE),
        }
    }

    /// `f` を実行し、その結果の部分木の高さ（現在の深さから到達した最大深さまでの段数）を返す
    fn measured<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> (T, usize) {
        let saved_peak = std::mem::replace(&mut self.peak, self.depth);
        let r = f(self);
        let height = self.peak - self.depth;
        self.peak = self.peak.max(saved_peak);
        (r, height)
    }

    /// 文パーサでブロック本体を解析する（ネスト 1 段）。到達した深さを引き継ぐ
    fn parse_nested_statements(
        &mut self,
    ) -> Result<(Vec<LocatedStatement>, Vec<CodeParseError>), usize> {
        self.nested(1, |b| {
            let (stats, errs, peak) = parse_to_statements_at(b.iter, b.depth);
            b.peak = b.peak.max(peak);
            (stats, errs)
        })
    }

    /// 左結合の二項演算子の連鎖を解析する
    ///
    /// `lower` は 1 段優先度の高いオペランドの解析、`op_of` はこの優先度の演算子の判定。
    /// 左結合の連鎖は左深の構文木になるため、各ノードの高さ
    /// （`max(左の高さ, 右の高さ) + 1`）を計測し、ネスト深度の上限を超えたらエラーにする。
    fn parse_left_assoc_chain(
        &mut self,
        lower: fn(&mut Self) -> Box<LocatedExpression>,
        op_of: fn(&Token) -> Option<Operator2>,
    ) -> Box<LocatedExpression> {
        let (mut left, mut height) = self.measured(lower);
        loop {
            let op = match self.iter.peek().and_then(|(token, _)| op_of(token)) {
                Some(op) => op,
                None => break,
            };
            self.iter.next();
            let (right, right_height) = self.measured(lower);
            height = height.max(right_height) + 1;
            let start = left.location.start;
            let end = right.location.end;
            if self.depth + height > MAX_NESTING_DEPTH {
                let e = self.add_nesting_error();
                return self.located(Expression::Invalid(e), start, end);
            }
            left = self.located(Expression::Operation2(op, left, right), start, end);
        }
        self.peak = self.peak.max(self.depth + height);
        left
    }

    /// 残りトークン数（peek 済みのトークンを含む）。式解析で入力が前進したかの判定に使う
    fn remaining_tokens(&self) -> usize {
        self.iter.len()
    }

    /// 引数の式解析で入力が前進しなかったときに、引数リスト全体を表す Invalid のエラーインデックスを返す
    ///
    /// 前進しなかった場合、factor のフォールバック分岐で既にエラーが記録されているため、
    /// その（引数の解析中に記録された最後の）エラーを再利用する。
    /// 引数の解析中にエラーが記録されていなければ新たに記録する。
    ///
    /// `errs_before`: 引数の式解析を始める前のエラー件数
    fn error_index_for_stalled_arg(&mut self, token_info: &TokenInfo, errs_before: usize) -> usize {
        if self.code_parse_error.len() > errs_before {
            self.code_parse_error.len() - 1
        } else {
            self.add_parse_error(token_info, "expected expression")
        }
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
                    // バグ修正: 引数の式が不正トークン（`;` `}` `]` など）で始まると、factor はエラーを
                    // 記録するだけでトークンを消費しない。そのため同じ位置で root を呼び続け、
                    // 無限ループとメモリ無制限消費が起きていた。
                    // 解決策: root 呼び出しで入力が前進しなかった場合は引数リストを打ち切り、
                    // 不正トークンは呼び出し元（文パーサ）の回復処理に任せる。
                    let before = self.remaining_tokens();
                    let errs_before = self.code_parse_error.len();
                    let e = self.parse_to_expression_tree_root();
                    if self.remaining_tokens() == before {
                        let err = self.error_index_for_stalled_arg(token_info, errs_before);
                        let end = self.current_pos();
                        return self.located(Expression::Invalid(err), start, end);
                    }
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

    /// 一次式（後置演算子を含まない factor）を解析する
    fn parse_to_expression_tree_primary(&mut self, start: usize) -> Box<LocatedExpression> {
        match self.iter.peek() {
            Some((Token::Keyword(Keyword::Struct), _)) => {
                self.iter.next();
                let name = match self.iter.next() {
                    Some((Token::Identifier(id), token_info)) => {
                        if !id
                            .chars()
                            .next()
                            .map(|c| c.is_ascii_uppercase())
                            .unwrap_or(false)
                        {
                            let e = self.add_parse_error(
                                token_info,
                                "struct name must start with an uppercase letter",
                            );
                            let end = self.current_pos();
                            return self.located(Expression::Invalid(e), start, end);
                        }
                        id.clone()
                    }
                    Some((_, token_info)) => {
                        let e = self.add_parse_error(token_info, "expected struct name");
                        let end = self.current_pos();
                        return self.located(Expression::Invalid(e), start, end);
                    }
                    None => {
                        let e = self.add_end_error("unexpected end of input");
                        let end = self.current_pos();
                        return self.located(Expression::Invalid(e), start, end);
                    }
                };

                if let Err(e) = match_expect_token!(self, self.iter.next(), Token::ParenthesisL) {
                    let end = self.current_pos();
                    return self.located(Expression::Invalid(e), start, end);
                }

                let mut args = Vec::<LocatedExpression>::new();
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
                                self.add_parse_error(token_info, "unexpected comma");
                            }
                            self.iter.next();
                            let end = self.current_pos();
                            return self.located(
                                Expression::StructLiteral(name.clone(), args),
                                start,
                                end,
                            );
                        }
                        Some((Token::Comma, token_info)) => {
                            if let State::Eval = state {
                                state = State::Comma;
                            } else {
                                self.add_parse_error(token_info, "unexpected comma");
                            }
                            self.iter.next();
                        }
                        Some((_, token_info)) => {
                            if let State::Eval = state {
                                self.add_parse_error(token_info, "missing comma");
                            }
                            // バグ修正: 引数の式が不正トークン（`;` `}` `]` など）で始まると、factor はエラーを
                            // 記録するだけでトークンを消費しない。そのため同じ位置で root を呼び続け、
                            // 無限ループとメモリ無制限消費が起きていた。
                            // 解決策: root 呼び出しで入力が前進しなかった場合は引数リストを打ち切り、
                            // 不正トークンは呼び出し元（文パーサ）の回復処理に任せる。
                            let before = self.remaining_tokens();
                            let errs_before = self.code_parse_error.len();
                            let e = self.parse_to_expression_tree_root();
                            if self.remaining_tokens() == before {
                                let err = self.error_index_for_stalled_arg(token_info, errs_before);
                                let end = self.current_pos();
                                return self.located(Expression::Invalid(err), start, end);
                            }
                            args.push((*e).clone());
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
                // 修飾識別子 A$B$x の処理: $ に続く識別子を貪欲に結合する
                let mut qualified_id = id;
                while let Some((Token::Dollar, _)) = self.iter.peek() {
                    self.iter.next(); // '$' を消費
                    match self.iter.next() {
                        Some((Token::Identifier(next_id), _)) => {
                            qualified_id = format!("{}${}", qualified_id, next_id);
                        }
                        Some((_, token_info)) => {
                            let e =
                                self.add_parse_error(token_info, "expected identifier after '$'");
                            let end = self.current_pos();
                            return self.located(Expression::Invalid(e), start, end);
                        }
                        None => {
                            let e = self.add_end_error("unexpected end of input after '$'");
                            let end = self.current_pos();
                            return self.located(Expression::Invalid(e), start, end);
                        }
                    }
                }
                let id = qualified_id;
                if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
                    self.parse_to_expression_tree_function_located(&id, start)
                } else if let Some((Token::BracketL, _)) = self.iter.peek() {
                    // 配列アクセス: arr[expr] → ArrayAccess として保持（意味解析での配列サイズ検証のため）
                    self.iter.next(); // '[' を消費
                    let index_expr = self.parse_to_expression_tree_root();
                    match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
                    let end = self.current_pos();
                    self.located(Expression::ArrayAccess(id, index_expr), start, end)
                } else {
                    let end = self.current_pos();
                    self.located(Expression::Variable(id), start, end)
                }
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
            // if/block を factor レベルで解析（while は文として parse_to_statements で処理）
            Some((Token::Keyword(Keyword::If), _)) => self.parse_to_expression_tree_if_impl(),
            Some((Token::BraceL, _)) => self.parse_to_expression_tree_block_impl(),
            Some((token, token_info)) => {
                let e = self
                    .add_parse_error(token_info, format!("unexpected token {}", token.describe()));
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

    fn parse_to_expression_tree_factor(&mut self) -> Box<LocatedExpression> {
        let start = self.current_pos();
        let (mut result, mut height) = self.measured(|b| b.parse_to_expression_tree_primary(start));

        // 後置演算子: (expr)[i] → *(&(expr) + i) に脱糖, expr @ type, expr.field
        // Identifier ケースの ArrayAccess (arr[i]) は match 内で既に処理されており、
        // ここでは括弧式・関数呼び出し・ArrayAccess 後の連鎖アクセスが対象となる。
        // 旧仕様 *(expr + i) では expr がポインタを返す前提だったが、
        // arr[i] = *(&arr + i) と一貫させるため *(&(expr) + i) に変更。
        //
        // 後置演算子の連鎖はループで左深の構文木を作るため、二項演算子の連鎖と同様に
        // 各ノードの高さを計測してネスト深度の上限を確認する。
        loop {
            if let Some((Token::BracketL, _)) = self.iter.peek() {
                self.iter.next(); // '[' を消費
                let (index_expr, index_height) =
                    self.measured(|b| b.parse_to_expression_tree_root());
                match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
                let end = self.current_pos();
                // Deref(Plus(Ref(result), index)) の 3 段
                height = (height + 1).max(index_height) + 2;
                // (expr)[i] → *(&(expr) + i) に脱糖
                let ref_expr =
                    self.located(Expression::Operation1(Operator1::Ref, result), start, end);
                let plus_expr = self.located(
                    Expression::Operation2(Operator2::Plus, ref_expr, index_expr),
                    start,
                    end,
                );
                result = self.located(
                    Expression::Operation1(Operator1::Deref, plus_expr),
                    start,
                    end,
                );
            } else if let Some((Token::At, _)) = self.iter.peek() {
                self.iter.next(); // '@'
                let type_spec = match self.parse_type_spec() {
                    Ok(ts) => ts,
                    Err(e) => {
                        let end = self.current_pos();
                        return self.located(Expression::Invalid(e), start, end);
                    }
                };
                let end = self.current_pos();
                height = height.max(type_spec_height(&type_spec)) + 1;
                result = self.located(Expression::TypeAnnotation(result, type_spec), start, end);
            } else if let Some((Token::Dot, _)) = self.iter.peek() {
                self.iter.next(); // '.'
                let field = match self.iter.next() {
                    Some((Token::Identifier(id), _)) => id.clone(),
                    Some((_, token_info)) => {
                        let e = self.add_parse_error(token_info, "expected field identifier");
                        let end = self.current_pos();
                        return self.located(Expression::Invalid(e), start, end);
                    }
                    None => {
                        let e = self.add_end_error("unexpected end of input");
                        let end = self.current_pos();
                        return self.located(Expression::Invalid(e), start, end);
                    }
                };

                if let Some((Token::BracketL, _)) = self.iter.peek() {
                    self.iter.next();
                    let (index_expr, index_height) =
                        self.measured(|b| b.parse_to_expression_tree_root());
                    match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
                    let end = self.current_pos();
                    height = height.max(index_height) + 1;
                    result = self.located(
                        Expression::FieldArrayAccess(result, field, index_expr),
                        start,
                        end,
                    );
                } else {
                    let end = self.current_pos();
                    height += 1;
                    result = self.located(Expression::FieldAccess(result, field), start, end);
                }
            } else {
                break;
            }
            if self.depth + height > MAX_NESTING_DEPTH {
                let e = self.add_nesting_error();
                let end = self.current_pos();
                return self.located(Expression::Invalid(e), start, end);
            }
        }

        self.peak = self.peak.max(self.depth + height);
        result
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
        // 単項演算子 1 個につきネスト 1 段（`----x` は深い構文木になる）
        let mut left = match self.nested(op_stack.len(), |b| b.parse_to_expression_tree_factor()) {
            Ok(e) => e,
            Err(e) => {
                let end = self.current_pos();
                return self.located(Expression::Invalid(e), start, end);
            }
        };
        while let Some(op) = op_stack.pop() {
            let end = left.location.end;
            left = self.located(Expression::Operation1(op, left), start, end);
        }
        left
    }

    fn parse_to_expression_tree_mul(&mut self) -> Box<LocatedExpression> {
        self.parse_left_assoc_chain(Self::parse_to_expression_tree_unary, |token| match token {
            Token::Asterisk => Some(Operator2::Multiply),
            Token::Slash => Some(Operator2::Divide),
            Token::Percent => Some(Operator2::Modulo),
            _ => None,
        })
    }

    fn parse_to_expression_tree_plus(&mut self) -> Box<LocatedExpression> {
        self.parse_left_assoc_chain(Self::parse_to_expression_tree_mul, |token| match token {
            Token::Plus => Some(Operator2::Plus),
            Token::Minus => Some(Operator2::Minus),
            _ => None,
        })
    }

    fn parse_to_expression_tree_compare(&mut self) -> Box<LocatedExpression> {
        self.parse_left_assoc_chain(Self::parse_to_expression_tree_plus, |token| match token {
            Token::DoubleEqual => Some(Operator2::Equal),
            Token::NotEqual => Some(Operator2::NotEqual),
            Token::Less => Some(Operator2::Less),
            Token::LessEqual => Some(Operator2::LessEqual),
            Token::Greater => Some(Operator2::Greater),
            Token::GreaterEqual => Some(Operator2::GreaterEqual),
            _ => None,
        })
    }

    fn parse_to_expression_tree_logical_and(&mut self) -> Box<LocatedExpression> {
        self.parse_left_assoc_chain(
            Self::parse_to_expression_tree_compare,
            |token| match token {
                Token::DoubleAmpersand => Some(Operator2::LogicalAnd),
                _ => None,
            },
        )
    }

    fn parse_to_expression_tree_logical_or(&mut self) -> Box<LocatedExpression> {
        self.parse_left_assoc_chain(
            Self::parse_to_expression_tree_logical_and,
            |token| match token {
                Token::DoublePipe => Some(Operator2::LogicalOr),
                _ => None,
            },
        )
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
        let right = match self.nested(1, |b| b.parse_to_expression_tree_assign()) {
            Ok(e) => e,
            Err(e) => {
                let end = self.current_pos();
                return self.located(Expression::Invalid(e), left.location.start, end);
            }
        };
        let start = left.location.start;
        let end = right.location.end;
        self.located(Expression::Operation2(op, left, right), start, end)
    }

    // ブロックスコープ式の解析処理
    fn parse_to_expression_tree_block_impl(&mut self) -> Box<LocatedExpression> {
        let start = self.current_pos();
        self.iter.next(); // '{' を消費
        let (stat, mut stat_err) = match self.parse_nested_statements() {
            Ok(r) => r,
            Err(e) => {
                let end = self.current_pos();
                return self.located(Expression::Invalid(e), start, end);
            }
        };
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
        let token = self.iter.next(); // if キーワードを消費（コロンも内包済み）
        assert!(matches!(token, Some((Token::Keyword(Keyword::If), _))));

        // Keyword トークンがコロンを内包済みのため、直接条件式をパース
        let cond = self.parse_to_expression_tree_root();
        if let Err(e) = match_expect_token!(self, self.iter.next(), Token::BraceL) {
            // NOTE: statements ではなく expression が来ても許容、でいいかもね?
            let end = self.current_pos();
            return self.located(Expression::Invalid(e), start, end);
        }

        let (stats_true, mut stats_err) = match self.parse_nested_statements() {
            Ok(r) => r,
            Err(e) => {
                let end = self.current_pos();
                return self.located(Expression::Invalid(e), start, end);
            }
        };
        if !stats_err.is_empty() {
            self.code_parse_error.append(&mut stats_err);
        }

        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);

        let stats_false = match self.iter.peek() {
            Some((Token::Keyword(Keyword::Else), token_info)) => {
                let else_start = token_info.code_pointer;
                self.iter.next(); // else キーワードを消費（コロンも内包済み）
                                  // Keyword トークンがコロンを内包済みのため、コロン消費は不要

                match self.iter.peek() {
                    Some((Token::Keyword(Keyword::If), _)) => {
                        // else: if: cond {}（連鎖 1 つにつきネスト 2 段: else 側の文と if 式の 2 段が構文木に積まれる）
                        let if_expr = match self.nested(2, |b| b.parse_to_expression_tree_if_impl())
                        {
                            Ok(e) => e,
                            Err(e) => {
                                let end = self.current_pos();
                                return self.located(Expression::Invalid(e), start, end);
                            }
                        };
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
                        let (stats, mut stats_err) = match self.parse_nested_statements() {
                            Ok(r) => r,
                            Err(e) => {
                                let end = self.current_pos();
                                return self.located(Expression::Invalid(e), start, end);
                            }
                        };
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
        // 部分式（括弧・引数・添字・if 条件など）はすべてここを経由するため、ここでネストを 1 段数える
        match self.nested(1, |b| b.parse_to_expression_tree_assign()) {
            Ok(e) => e,
            Err(e) => {
                let pos = self.current_pos();
                self.located(Expression::Invalid(e), pos, pos)
            }
        }
    }

    fn parse_type_spec(&mut self) -> Result<TypeSpec, usize> {
        let mut ref_depth = 0usize;
        loop {
            match self.iter.peek() {
                Some((Token::Ampersand, _)) => {
                    self.iter.next();
                    ref_depth += 1;
                }
                Some((Token::DoubleAmpersand, _)) => {
                    self.iter.next();
                    ref_depth += 2;
                }
                _ => break,
            }
        }
        // 型指定も参照・配列の段数だけ深い構文木になるため、ネスト深度の上限に含める。
        // 深い構文木を作ってから破棄すると drop の再帰でスタックを使い切るため、構築前に確認する
        if self.depth + ref_depth > MAX_NESTING_DEPTH {
            return Err(self.add_nesting_error());
        }
        let mut array_dims = 0usize;

        let mut base = match self.iter.peek() {
            Some((Token::Identifier(id), _)) if id == "int" => {
                self.iter.next();
                TypeSpec::Int
            }
            Some((Token::Identifier(id), _)) if id == "void" => {
                self.iter.next();
                TypeSpec::Void
            }
            Some((Token::Identifier(id), token_info)) => {
                let name = id.clone();
                self.iter.next();
                if !name
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
                {
                    let e = self.add_parse_error(token_info, "expected type specifier");
                    return Err(e);
                }
                TypeSpec::Named(name)
            }
            Some((_, token_info)) => {
                let e = self.add_parse_error(token_info, "expected type specifier");
                return Err(e);
            }
            None => {
                let e = self.add_end_error("unexpected end of input");
                return Err(e);
            }
        };

        while let Some((Token::BracketL, _)) = self.iter.peek() {
            self.iter.next();
            let size = match self.iter.next() {
                Some((Token::Number(n), token_info)) if *n > 0 => {
                    // バグ: `*n as usize` は usize が 32bit の環境（wasm32）で上位ビットを切り捨て、
                    // 宣言と異なる配列サイズになっていた。
                    // 解決策: usize::try_from で変換し、収まらない場合は parse error にする。
                    match usize::try_from(*n) {
                        Ok(size) => size,
                        Err(_) => {
                            let e = self.add_parse_error(token_info, "array size too large");
                            return Err(e);
                        }
                    }
                }
                Some((_, token_info)) => {
                    let e = self.add_parse_error(token_info, "expected array size");
                    return Err(e);
                }
                None => {
                    let e = self.add_end_error("unexpected end of input");
                    return Err(e);
                }
            };
            match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
            array_dims += 1;
            if self.depth + ref_depth + array_dims > MAX_NESTING_DEPTH {
                return Err(self.add_nesting_error());
            }
            base = TypeSpec::Array(Box::new(base), size);
        }

        let mut spec = base;
        for _ in 0..ref_depth {
            spec = TypeSpec::Ref(Box::new(spec));
        }
        Ok(spec)
    }
}

/// 型指定の構文木の高さ（参照・配列の段数）
pub(super) fn type_spec_height(spec: &TypeSpec) -> usize {
    let mut height = 0;
    let mut current = spec;
    loop {
        match current {
            TypeSpec::Ref(inner) | TypeSpec::Array(inner, _) => {
                height += 1;
                current = inner;
            }
            TypeSpec::Int | TypeSpec::Void | TypeSpec::Named(_) => return height,
        }
    }
}

#[cfg(test)]
pub(super) fn parse_to_expression_tree_root(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> (Box<LocatedExpression>, Vec<CodeParseError>) {
    let (e, errs, _) = ExpressionBuilder::parse(iter, 0);
    (e, errs)
}

/// ネスト深度 `depth` の位置にある式を解析する（文パーサから呼ばれる）
///
/// 戻り値の 3 番目は解析中に到達したネスト深度の最大値
pub(super) fn parse_to_expression_tree_at(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
    depth: usize,
) -> (Box<LocatedExpression>, Vec<CodeParseError>, usize) {
    ExpressionBuilder::parse(iter, depth)
}

#[cfg(test)]
mod test;
