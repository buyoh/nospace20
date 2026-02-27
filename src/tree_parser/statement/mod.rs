use std::iter;

use crate::{
    base::{CodeParseError, SourceLocation},
    code_parse_error,
    token_parser::{Keyword, PrettyToken, Token, TokenInfo},
};

use super::expression::*;

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

#[derive(Clone, Debug)]
pub enum Statement {
    VariableDeclaration(String, Box<LocatedExpression>, bool, Option<i64>), // (name, init_expr, is_static, array_size)
    FunctionDeclaration(String, Vec<String>, Vec<LocatedStatement>),
    Continue,
    Break,
    Return(Option<Box<LocatedExpression>>),
    While(Box<LocatedExpression>, Vec<LocatedStatement>),  // while 文
    Expression(Box<LocatedExpression>),
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

    /// セミコロンまでトークンをスキップし、セミコロン自体も消費する。
    /// エラーリカバリで多用するパターンを共通化したもの。
    fn skip_to_semicolon(&mut self) {
        while let Some((token, _)) = self.iter.peek() {
            if matches!(token, Token::Semicolon) {
                break;
            }
            self.iter.next();
        }
        self.iter.next(); // セミコロンを消費
    }

    /// 現在のピーク位置の `code_pointer` を返す。トークンがなければ `default` を返す。
    fn current_pos_or(&mut self, default: usize) -> usize {
        self.iter
            .peek()
            .map(|(_, info)| info.code_pointer)
            .unwrap_or(default)
    }

    fn parse_to_statements_block(&mut self) -> Vec<LocatedStatement> {
        match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
        let ss = self.parse_to_statements();
        match_expect_token_unused!(self, self.iter.next(), Token::BraceR);
        ss
    }

    /// `let:` または `static:` キーワードを消費して変数宣言をパースする。
    /// 呼び出し元が `is_static` フラグを渡すことで let/static の両方を統一的に扱う。
    fn parse_to_statements_variable(&mut self, start_pos: usize, is_static: bool) -> Vec<LocatedStatement> {
        // 呼び出し元が既にキーワードを確認済みなので、そのまま消費する
        self.iter.next();
        self.parse_variable_declarations(start_pos, is_static)
    }

    /// 配列サイズ `[N]` または `[]` をパースする。
    ///
    /// 戻り値:
    /// - `Some((bracket_specified, array_size))`: 正常にパース完了
    /// - `None`: エラー発生（`results` にエラー文が追加済み、呼び出し元は `skip_to_semicolon` 後に return すること）
    fn parse_array_size(
        &mut self,
        start_pos: usize,
        results: &mut Vec<LocatedStatement>,
    ) -> Option<(bool, Option<i64>)> {
        if let Some((Token::BracketL, _)) = self.iter.peek() {
            self.iter.next(); // '[' を消費

            if let Some((Token::BracketR, _)) = self.iter.peek() {
                // "[]" - サイズ省略（初期化から推論）
                self.iter.next(); // ']' を消費
                return Some((true, None));
            }

            // "[N]" - サイズ指定（定数のみ）
            let size = match self.iter.next() {
                Some((Token::Number(n), token_info)) => {
                    if *n <= 0 {
                        // エラー: 配列サイズは正の整数でなければならない
                        // Quality-1: エラー位置をサイズ値のトークン位置に修正
                        let err_pos = token_info.code_pointer;
                        let err_idx = self
                            .add_parse_error(&TokenInfo { code_pointer: err_pos }, "array size must be positive");
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        return None;
                    }
                    *n
                }
                Some((_, token_info)) => {
                    let err_idx =
                        self.add_parse_error(token_info, "expected array size or ']'");
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    });
                    return None;
                }
                None => {
                    let err_idx = self.add_end_error("unexpected end of input");
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    });
                    return None;
                }
            };

            // ']' を消費
            match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
            Some((true, Some(size)))
        } else {
            Some((false, None))
        }
    }

    /// 配列の文字列初期化 `("Hello")` をパースして各代入文を `results` へ追加する。
    /// `(` は呼び出し元で消費済み、`StringLiteral` トークンがピーク位置にあること。
    ///
    /// 戻り値: `false` はエラー発生（`results` にエラー文追加済み、呼び出し元は return すること）
    fn parse_array_string_init(
        &mut self,
        id: &str,
        start_pos: usize,
        array_size: Option<i64>,
        is_static: bool,
        results: &mut Vec<LocatedStatement>,
    ) -> bool {
        let chars = match self.iter.peek() {
            Some((Token::StringLiteral(chars), _)) => {
                let chars = chars.clone();
                self.iter.next(); // StringLiteral を消費
                chars
            }
            _ => unreachable!("parse_array_string_init called without StringLiteral"),
        };

        // ")" を消費
        match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

        // 文字列サイズ = 文字数 + 1（ヌル終端）
        let string_size = (chars.len() + 1) as i64;

        // サイズチェック or 推論
        let actual_size = if let Some(explicit_size) = array_size {
            if string_size > explicit_size {
                // Quality-1: エラー位置は start_pos（宣言の先頭）のまま適切
                let err_idx = self.add_parse_error(
                    &TokenInfo { code_pointer: start_pos },
                    format!(
                        "string literal too long for array of size {}: needs {}",
                        explicit_size, string_size
                    ),
                );
                results.push(LocatedStatement {
                    statement: Statement::Invalid(err_idx),
                    location: SourceLocation::from_single(start_pos),
                });
                return false;
            }
            explicit_size
        } else {
            // '[]' の場合: 文字列長から推論
            string_size
        };

        let end_pos = self.current_pos_or(start_pos);
        let loc = SourceLocation::new(start_pos, end_pos);

        // 配列宣言を追加
        results.push(LocatedStatement {
            statement: Statement::VariableDeclaration(
                id.to_string(),
                Box::new(LocatedExpression {
                    expression: Expression::Factor(0),
                    location: loc.clone(),
                }),
                is_static,
                Some(actual_size),
            ),
            location: loc.clone(),
        });

        // 各文字を配列要素に代入
        let null_idx = chars.len();
        for (i, char_val) in chars.into_iter().enumerate() {
            let assign_expr = Box::new(LocatedExpression {
                expression: Expression::Operation2(
                    Operator2::Assign,
                    Box::new(LocatedExpression {
                        expression: Expression::ArrayAccess(
                            id.to_string(),
                            Box::new(LocatedExpression {
                                expression: Expression::Factor(i as i64),
                                location: loc.clone(),
                            }),
                        ),
                        location: loc.clone(),
                    }),
                    Box::new(LocatedExpression {
                        expression: Expression::Factor(char_val),
                        location: loc.clone(),
                    }),
                ),
                location: loc.clone(),
            });
            results.push(LocatedStatement {
                statement: Statement::Expression(assign_expr),
                location: loc.clone(),
            });
        }

        // ヌル終端を追加
        let assign_expr = Box::new(LocatedExpression {
            expression: Expression::Operation2(
                Operator2::Assign,
                Box::new(LocatedExpression {
                    expression: Expression::ArrayAccess(
                        id.to_string(),
                        Box::new(LocatedExpression {
                            expression: Expression::Factor(null_idx as i64),
                            location: loc.clone(),
                        }),
                    ),
                    location: loc.clone(),
                }),
                Box::new(LocatedExpression {
                    expression: Expression::Factor(0),
                    location: loc.clone(),
                }),
            ),
            location: loc.clone(),
        });
        results.push(LocatedStatement {
            statement: Statement::Expression(assign_expr),
            location: loc.clone(),
        });

        true
    }

    /// 配列のリスト初期化 `([val1, val2, ...])` をパースして各代入文を `results` へ追加する。
    /// `(` および `[` は呼び出し元で消費済みであること。
    ///
    /// 戻り値: `false` はエラー発生（`results` にエラー文追加済み、呼び出し元は return すること）
    fn parse_array_list_init(
        &mut self,
        id: &str,
        start_pos: usize,
        array_size: Option<i64>,
        is_static: bool,
        results: &mut Vec<LocatedStatement>,
    ) -> bool {
        let mut init_values = Vec::new();

        // ']' になるまで初期化値を読み取る
        loop {
            if let Some((Token::BracketR, _)) = self.iter.peek() {
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

        // ']' を消費
        match_expect_token_unused!(self, self.iter.next(), Token::BracketR);
        // ')' を消費
        match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

        // 空の初期化リストはエラー
        if init_values.is_empty() {
            let err_idx = self.add_parse_error(
                &TokenInfo { code_pointer: start_pos },
                "empty initializer list: array size cannot be 0",
            );
            results.push(LocatedStatement {
                statement: Statement::Invalid(err_idx),
                location: SourceLocation::from_single(start_pos),
            });
            return false;
        }

        // サイズチェック or 推論
        let actual_size = if let Some(explicit_size) = array_size {
            if init_values.len() > explicit_size as usize {
                let err_idx = self.add_parse_error(
                    &TokenInfo { code_pointer: start_pos },
                    format!(
                        "too many initializers for array of size {}: got {}",
                        explicit_size,
                        init_values.len()
                    ),
                );
                results.push(LocatedStatement {
                    statement: Statement::Invalid(err_idx),
                    location: SourceLocation::from_single(start_pos),
                });
                return false;
            }
            explicit_size
        } else {
            // '[]' の場合: リスト長から推論
            init_values.len() as i64
        };

        // 配列宣言を追加
        let end_pos = self.current_pos_or(start_pos);
        let loc = SourceLocation::new(start_pos, end_pos);

        results.push(LocatedStatement {
            statement: Statement::VariableDeclaration(
                id.to_string(),
                Box::new(LocatedExpression {
                    expression: Expression::Factor(0),
                    location: loc.clone(),
                }),
                is_static,
                Some(actual_size),
            ),
            location: loc.clone(),
        });

        // 各要素への代入文を生成: arr[0] = val0, arr[1] = val1, ...
        for (i, val_expr) in init_values.into_iter().enumerate() {
            let val_loc = val_expr.location.clone();
            let assign_expr = Box::new(LocatedExpression {
                expression: Expression::Operation2(
                    Operator2::Assign,
                    Box::new(LocatedExpression {
                        expression: Expression::ArrayAccess(
                            id.to_string(),
                            Box::new(LocatedExpression {
                                expression: Expression::Factor(i as i64),
                                location: loc.clone(),
                            }),
                        ),
                        location: loc.clone(),
                    }),
                    val_expr,
                ),
                location: SourceLocation::new(loc.start, val_loc.end),
            });
            results.push(LocatedStatement {
                statement: Statement::Expression(assign_expr),
                location: loc.clone(),
            });
        }

        true
    }

    /// 通常変数の初期化 `(expr)` をパースして宣言文を `results` へ追加する。
    /// `(` は呼び出し元で消費済みであること。
    fn parse_variable_init(
        &mut self,
        id: &str,
        start_pos: usize,
        is_static: bool,
        results: &mut Vec<LocatedStatement>,
    ) {
        let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
        self.code_parse_error.append(&mut errs);

        // ")" を消費
        match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

        let end_pos = self.current_pos_or(start_pos);
        let loc = SourceLocation::new(start_pos, end_pos);

        // 代入式を構築: id = expr
        let expr_loc = expr.location.clone();
        let init_expr = Box::new(LocatedExpression {
            expression: Expression::Operation2(
                Operator2::Assign,
                Box::new(LocatedExpression {
                    expression: Expression::Variable(id.to_string()),
                    location: loc.clone(),
                }),
                expr,
            ),
            location: SourceLocation::new(start_pos, expr_loc.end),
        });

        results.push(LocatedStatement {
            statement: Statement::VariableDeclaration(id.to_string(), init_expr, is_static, None),
            location: loc,
        });
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
            let (bracket_specified, array_size) =
                match self.parse_array_size(start_pos, &mut results) {
                    Some(v) => v,
                    None => {
                        self.skip_to_semicolon();
                        return results;
                    }
                };

            // 初期化式のチェック
            if let Some((Token::ParenthesisL, _)) = self.iter.peek() {
                // "(" を消費
                self.iter.next();

                if bracket_specified {
                    // 配列の初期化: ("string") または ([val1, val2, ...])

                    if let Some((Token::StringLiteral(_), _)) = self.iter.peek() {
                        // 文字列初期化: ("Hello")
                        let ok = self.parse_array_string_init(
                            &id, start_pos, array_size, is_static, &mut results,
                        );
                        if !ok {
                            self.skip_to_semicolon();
                            return results;
                        }
                    } else if let Some((Token::BracketL, _)) = self.iter.peek() {
                        // 数値リスト初期化: ([val1, val2, val3])
                        self.iter.next(); // '[' を消費
                        let ok = self.parse_array_list_init(
                            &id, start_pos, array_size, is_static, &mut results,
                        );
                        if !ok {
                            self.skip_to_semicolon();
                            return results;
                        }
                    } else {
                        // エラー: 配列初期化には '[' か文字列リテラルが必要
                        let err_idx = self.add_parse_error(
                            &TokenInfo { code_pointer: start_pos },
                            "expected '[' or string literal for array initialization",
                        );
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        self.skip_to_semicolon();
                        return results;
                    }
                } else {
                    // 通常変数の初期化: (expr)
                    self.parse_variable_init(&id, start_pos, is_static, &mut results);
                }
            } else {
                // 初期化式なし
                if bracket_specified && array_size.is_none() {
                    // エラー: '[]' でサイズ省略しているのに初期値なし
                    let err_idx = self.add_parse_error(
                        &TokenInfo { code_pointer: start_pos },
                        "array size not specified and no initializer: use '[N]' or '[]([...])' or '[](...)'",
                    );
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    });
                    self.skip_to_semicolon();
                    return results;
                }

                let end_pos = self.current_pos_or(start_pos);
                let loc = SourceLocation::new(start_pos, end_pos);

                results.push(LocatedStatement {
                    statement: Statement::VariableDeclaration(
                        id.clone(),
                        Box::new(LocatedExpression {
                            expression: Expression::Factor(0),
                            location: loc.clone(),
                        }),
                        is_static,
                        array_size,
                    ),
                    location: loc,
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
        // 呼び出し元が既に Token::Keyword(Keyword::Func) を確認済み
        self.iter.next();
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
                    // Quality-5: trailing comma `f(x,)` のエラーメッセージを改善
                    // State::Comma の場合は最後のカンマの位置でエラーを出すべきだが、
                    // 現状ではカンマを消費した時点で State::Comma に遷移し ')' の位置でエラーとなる。
                    // ここでは `)` の位置で "trailing ','" として報告する。
                    if let State::Comma = state {
                        self.add_parse_error(token_info, "trailing ','");
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
        let end_pos = self.current_pos_or(start_pos);
        LocatedStatement {
            statement: Statement::FunctionDeclaration(id.clone(), args, body),
            location: SourceLocation::new(start_pos, end_pos),
        }
    }

    fn parse_to_statements_return(&mut self, start_pos: usize) -> LocatedStatement {
        // 呼び出し元が既に Token::Keyword(Keyword::Return) を確認済み
        self.iter.next();
        // return: expr; または return:; (void return)
        // コロンの後にセミコロンが来たら void return
        if let Some(token) = self.iter.peek() {
            if matches!(token.0, Token::Semicolon) {
                // return; (コロンなし)
                let end_pos = self.current_pos_or(start_pos);
                self.iter.next(); // consume semicolon
                return LocatedStatement {
                    statement: Statement::Return(None),
                    location: SourceLocation::new(start_pos, end_pos),
                };
            }
        }
        match_expect_token_unused!(self, self.iter.next(), Token::Colon);
        // return: の後にセミコロンが来たら void return
        if let Some(token) = self.iter.peek() {
            if matches!(token.0, Token::Semicolon) {
                let end_pos = self.current_pos_or(start_pos);
                self.iter.next(); // consume semicolon
                return LocatedStatement {
                    statement: Statement::Return(None),
                    location: SourceLocation::new(start_pos, end_pos),
                };
            }
        }
        let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
        self.code_parse_error.append(&mut errs);
        let end_pos = self.current_pos_or(start_pos);
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        LocatedStatement {
            statement: Statement::Return(Some(expr)),
            location: SourceLocation::new(start_pos, end_pos),
        }
    }

    fn parse_to_statements(&mut self) -> Vec<LocatedStatement> {
        let mut statements = Vec::<LocatedStatement>::new();
        while let Some(token) = self.iter.peek() {
            let start_pos = token.1.code_pointer;
            match &token.0 {
                Token::Keyword(Keyword::Let) => {
                    statements.extend(self.parse_to_statements_variable(start_pos, false));
                    continue;
                }
                Token::Keyword(Keyword::Static) => {
                    statements.extend(self.parse_to_statements_variable(start_pos, true));
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
                    let end_pos = self.current_pos_or(start_pos);
                    statements.push(LocatedStatement {
                        statement: Statement::Break,
                        location: SourceLocation::new(start_pos, end_pos),
                    });
                    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                    continue;
                }
                Token::Keyword(Keyword::Continue) => {
                    self.iter.next();
                    let end_pos = self.current_pos_or(start_pos);
                    statements.push(LocatedStatement {
                        statement: Statement::Continue,
                        location: SourceLocation::new(start_pos, end_pos),
                    });
                    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                    continue;
                }
                Token::Keyword(Keyword::While) => {
                    self.iter.next(); // while キーワードを消費

                    // ':' を期待
                    if let Err(_) = match_expect_token!(self, self.iter.next(), Token::Colon) {
                        self.skip_to_semicolon();
                        continue;
                    }

                    // 条件式をパース
                    let (cond, mut cond_errors) = parse_to_expression_tree_root(self.iter);
                    if !cond_errors.is_empty() {
                        self.code_parse_error.append(&mut cond_errors);
                    }

                    // '{' を期待
                    match_expect_token_unused!(self, self.iter.next(), Token::BraceL);
                    let (body, mut body_errors) = parse_to_statements(self.iter);
                    if !body_errors.is_empty() {
                        self.code_parse_error.append(&mut body_errors);
                    }
                    match_expect_token_unused!(self, self.iter.next(), Token::BraceR);

                    // ';' を消費
                    match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);

                    let end_pos = self.current_pos_or(start_pos);
                    statements.push(LocatedStatement {
                        statement: Statement::While(cond, body),
                        location: SourceLocation::new(start_pos, end_pos),
                    });
                    continue;
                }
                Token::BraceR => {
                    break;
                }
                _ => {}
            }
            let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
            self.code_parse_error.append(&mut errs);
            let end_pos = self.current_pos_or(start_pos);
            statements.push(LocatedStatement {
                statement: Statement::Expression(expr),
                location: SourceLocation::new(start_pos, end_pos),
            });
            match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        }
        statements
    }
}

pub(super) fn parse_to_statements(
    iter: &mut iter::Peekable<std::slice::Iter<PrettyToken>>,
) -> (Vec<LocatedStatement>, Vec<CodeParseError>) {
    StatementBuilder::parse(iter)
}

#[cfg(test)]
mod test;
