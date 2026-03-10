use std::iter;

use crate::{
    base::{CodeParseError, SourceLocation},
    code_parse_error,
    token_parser::{Keyword, PrettyToken, Token, TokenInfo},
};

use super::expression::*;

// マクロは macros.rs で定義され、mod.rs で #[macro_use] によりインポートされる

/// テンプレート関数の alias パラメータの種類
#[derive(Clone, Debug, PartialEq)]
pub enum AliasParamKind {
    /// `alias: func: name(arg1, arg2, ...)` — 関数パラメータ（引数名リスト付き）
    Func(Vec<String>),
    /// `alias: constexpr: name` — コンパイル時定数パラメータ
    Constexpr,
    /// `alias: static: name` — static 変数参照パラメータ（外部 static 変数への読み書きアクセス）
    Static,
}

/// テンプレート関数の alias パラメータ定義
#[derive(Clone, Debug, PartialEq)]
pub struct AliasParam {
    pub name: String,
    pub kind: AliasParamKind,
}

/// テンプレートインスタンス化時の alias 引数
#[derive(Clone, Debug, PartialEq)]
pub enum AliasArg {
    /// 関数名や変数名（識別子）
    Identifier(String),
    /// 整数リテラル
    Value(i64),
}

#[derive(Clone, Debug)]
pub enum Statement {
    VariableDeclaration(
        String,
        Box<LocatedExpression>,
        bool,
        bool,
        Option<i64>,
        Option<TypeSpec>,
    ), // (name, init_expr, is_static, is_final, array_size, type_annotation)
    /// コンパイル時定数定義: `constexpr: name(expr);`
    /// スタックスロットを確保せず、コンパイル時に定数値に解決される
    ConstexprDeclaration(String, Box<LocatedExpression>), // (name, expr)
    /// 識別子エイリアス定義: `alias: name(target);`
    /// コンパイル時に名前を target に解決する
    AliasIdentifier(String, String), // (name, target)
    /// ブロックエイリアス定義: `alias: name { 文... };`
    /// コンパイル時にブロック AST を名前に紐付け、呼び出し時にインライン展開する
    AliasBlock(String, Vec<LocatedStatement>), // (name, body)
    FunctionDeclaration(
        String,
        Vec<(String, Option<TypeSpec>)>,
        Vec<LocatedStatement>,
        Option<TypeSpec>,
    ),
    /// テンプレート関数定義: `func: name(args), alias: kind: param ... { body }`
    /// インスタンス化（AliasInstantiation）時に初めて具体的な関数が生成される。
    /// テンプレート自体はコード生成の対象にならない。
    TemplateFunctionDefinition {
        name: String,
        args: Vec<(String, Option<TypeSpec>)>,
        alias_params: Vec<AliasParam>,
        body: Vec<LocatedStatement>,
        return_type: Option<TypeSpec>,
    },
    /// テンプレートインスタンス化: `alias: new_name(template_name, arg1, ...);`
    /// semantic_analyzer の pre-pass で FunctionDeclaration に展開される。
    AliasInstantiation {
        name: String,
        template_name: String,
        alias_args: Vec<AliasArg>,
    },
    Continue,
    Break,
    Return(Option<Box<LocatedExpression>>),
    While(Box<LocatedExpression>, Vec<LocatedStatement>), // while 文
    /// for 文: (init block, cond block, step block, body block)
    /// repeat は tree_parser 段階で For に脱糖される
    For(
        Vec<LocatedStatement>,
        Vec<LocatedStatement>,
        Vec<LocatedStatement>,
        Vec<LocatedStatement>,
    ),
    Expression(Box<LocatedExpression>),
    /// 名前空間宣言: `namespace: Name { 文... }`
    /// 末尾セミコロンは不要（func: と同様の扱い）
    NamespaceDeclaration(String, Vec<LocatedStatement>),
    /// 構造体定義: struct: Name (field: type, ...);
    /// フィールドの型は省略可能（省略時は Int）
    StructDeclaration(String, Vec<StructFieldDecl>),
    Invalid(usize), // See, Expression::Invalid
}

/// 構造体フィールド宣言
#[derive(Clone, Debug)]
pub struct StructFieldDecl {
    pub name: String,
    pub type_spec: Option<TypeSpec>,
    pub array_size: Option<usize>,
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
    fn parse_to_statements_variable(
        &mut self,
        start_pos: usize,
        is_static: bool,
    ) -> Vec<LocatedStatement> {
        // 呼び出し元が既にキーワードを確認済みなので、そのまま消費する
        self.iter.next();
        self.parse_variable_declarations(start_pos, is_static, false)
    }

    /// `final:` キーワードを消費して final 変数宣言をパースする。
    /// `final: name(expr);` のように複数定義にも対応。
    fn parse_to_statements_final_variable(&mut self, start_pos: usize) -> Vec<LocatedStatement> {
        self.iter.next(); // Final キーワードを消費
        self.parse_variable_declarations(start_pos, false, true) // is_static=false, is_final=true
    }

    /// `constexpr:` キーワードを消費して定数定義をパースする。
    /// `constexpr: name(expr), name2(expr2);` のように複数定義にも対応。
    fn parse_constexpr_declarations(&mut self, start_pos: usize) -> Vec<LocatedStatement> {
        self.iter.next(); // Constexpr キーワードを消費

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
                    self.skip_to_semicolon();
                    return results;
                }
            };

            // 識別子の直後のトークンで式形式かブロック形式かを決定
            match self.iter.peek() {
                Some((Token::BraceL, _)) => {
                    // ブロック形式: constexpr: name { 文... };
                    let body = self.parse_to_statements_block();
                    let end_pos = self.current_pos_or(start_pos);
                    let loc = SourceLocation::new(start_pos, end_pos);
                    results.push(LocatedStatement {
                        statement: Statement::ConstexprDeclaration(
                            id.to_string(),
                            Box::new(LocatedExpression {
                                expression: Expression::Block(body),
                                location: loc.clone(),
                            }),
                        ),
                        location: loc,
                    });
                    // ブロック形式は単一定義 → ループを抜けてセミコロンを消費
                    break;
                }
                Some((Token::ParenthesisL, _)) => {
                    // 式形式: constexpr: name(expr)
                    self.iter.next(); // '(' を消費

                    // 定数式をパース（')' の直前まで）
                    let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
                    self.code_parse_error.append(&mut errs);

                    // ')' を消費
                    match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

                    let end_pos = self.current_pos_or(start_pos);
                    let loc = SourceLocation::new(start_pos, end_pos);

                    results.push(LocatedStatement {
                        statement: Statement::ConstexprDeclaration(id.to_string(), expr),
                        location: loc,
                    });

                    // ',' または ';' を確認
                    match self.iter.peek() {
                        Some((Token::Comma, _)) => {
                            self.iter.next(); // ',' を消費して次の定義へ
                        }
                        _ => break, // ';' または予期しないトークン → ループを抜ける
                    }
                }
                Some((_, token_info)) => {
                    let err_idx = self.add_parse_error(
                        token_info,
                        "expected '(' or '{' after constexpr identifier",
                    );
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    });
                    self.skip_to_semicolon();
                    return results;
                }
                None => {
                    let err_idx =
                        self.add_end_error("unexpected end of input in constexpr declaration");
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    });
                    return results;
                }
            }
        }

        // ';' を消費
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);

        results
    }

    /// `alias:` キーワードを消費して識別子エイリアスまたはブロックエイリアス定義をパースする。
    ///
    /// - `alias: name(target), name2(target2);` → 識別子エイリアス（複数定義可）
    /// - `alias: name { 文... };` → ブロックエイリアス（単一定義のみ）
    /// - `alias: name(template, arg1, arg2, ...);` → テンプレートインスタンス化（引数 2 つ以上）
    fn parse_alias_declarations(&mut self, start_pos: usize) -> Vec<LocatedStatement> {
        self.iter.next(); // Alias キーワードを消費

        let mut results = Vec::<LocatedStatement>::new();

        loop {
            // エイリアス名を取得
            let name = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id)
            {
                Ok(x) => x,
                Err(e) => {
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(e),
                        location: SourceLocation::from_single(start_pos),
                    });
                    self.skip_to_semicolon();
                    return results;
                }
            };

            // 名前の直後のトークンで識別子エイリアスかブロックエイリアスかを決定
            match self.iter.peek() {
                Some((Token::BraceL, _)) => {
                    // ブロックエイリアス: alias: name { 文... };
                    let body = self.parse_to_statements_block();
                    let end_pos = self.current_pos_or(start_pos);
                    let loc = SourceLocation::new(start_pos, end_pos);
                    results.push(LocatedStatement {
                        statement: Statement::AliasBlock(name.to_string(), body),
                        location: loc,
                    });
                    // ブロックエイリアスは単一定義 → ループを抜けてセミコロンを消費
                    break;
                }
                Some((Token::ParenthesisL, _)) => {
                    // `(` を消費
                    self.iter.next();

                    // 最初の引数（識別子）を取得
                    let first_target = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id)
                    {
                        Ok(x) => x,
                        Err(e) => {
                            results.push(LocatedStatement {
                                statement: Statement::Invalid(e),
                                location: SourceLocation::from_single(start_pos),
                            });
                            self.skip_to_semicolon();
                            return results;
                        }
                    };

                    // 次のトークンで単一引数か複数引数かを判定
                    match self.iter.peek() {
                        Some((Token::Comma, _)) => {
                            // 複数引数 → テンプレートインスタンス化
                            // alias: name(template_name, arg1, arg2, ...)
                            let mut alias_args: Vec<AliasArg> = Vec::new();
                            while let Some((Token::Comma, _)) = self.iter.peek() {
                                self.iter.next(); // ',' を消費
                                                  // alias 引数: 識別子 または 整数
                                match self.iter.next() {
                                    Some((Token::Identifier(arg_id), _)) => {
                                        alias_args.push(AliasArg::Identifier(arg_id.clone()));
                                    }
                                    Some((Token::Number(n), _)) => {
                                        alias_args.push(AliasArg::Value(*n));
                                    }
                                    Some((_, token_info)) => {
                                        let err_idx = self.add_parse_error(
                                            token_info,
                                            "expected identifier or integer as alias argument",
                                        );
                                        results.push(LocatedStatement {
                                            statement: Statement::Invalid(err_idx),
                                            location: SourceLocation::from_single(start_pos),
                                        });
                                        self.skip_to_semicolon();
                                        return results;
                                    }
                                    None => {
                                        let err_idx = self.add_end_error(
                                            "unexpected end of input in alias instantiation",
                                        );
                                        results.push(LocatedStatement {
                                            statement: Statement::Invalid(err_idx),
                                            location: SourceLocation::from_single(start_pos),
                                        });
                                        return results;
                                    }
                                }
                            }
                            // ')' を消費
                            match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);

                            let end_pos = self.current_pos_or(start_pos);
                            let loc = SourceLocation::new(start_pos, end_pos);
                            results.push(LocatedStatement {
                                statement: Statement::AliasInstantiation {
                                    name: name.to_string(),
                                    template_name: first_target.to_string(),
                                    alias_args,
                                },
                                location: loc,
                            });

                            // テンプレートインスタンス化は単一定義 → ループを抜ける
                            break;
                        }
                        Some((Token::ParenthesisR, _)) => {
                            // 単一引数 → 識別子エイリアス (既存の動作)
                            self.iter.next(); // ')' を消費

                            let end_pos = self.current_pos_or(start_pos);
                            let loc = SourceLocation::new(start_pos, end_pos);

                            results.push(LocatedStatement {
                                statement: Statement::AliasIdentifier(
                                    name.to_string(),
                                    first_target.to_string(),
                                ),
                                location: loc,
                            });

                            // ',' または ';' を確認
                            match self.iter.peek() {
                                Some((Token::Comma, _)) => {
                                    self.iter.next(); // ',' を消費して次の定義へ
                                }
                                _ => break, // ';' または予期しないトークン → ループを抜ける
                            }
                        }
                        Some((_, token_info)) => {
                            let err_idx = self.add_parse_error(
                                token_info,
                                "expected ',' or ')' after alias target",
                            );
                            results.push(LocatedStatement {
                                statement: Statement::Invalid(err_idx),
                                location: SourceLocation::from_single(start_pos),
                            });
                            self.skip_to_semicolon();
                            return results;
                        }
                        None => {
                            let err_idx =
                                self.add_end_error("unexpected end of input in alias declaration");
                            results.push(LocatedStatement {
                                statement: Statement::Invalid(err_idx),
                                location: SourceLocation::from_single(start_pos),
                            });
                            return results;
                        }
                    }
                }
                Some((_, token_info)) => {
                    let err_idx = self
                        .add_parse_error(token_info, "expected '(' or '{' after alias identifier");
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    });
                    self.skip_to_semicolon();
                    return results;
                }
                None => {
                    let err_idx =
                        self.add_end_error("unexpected end of input in alias declaration");
                    results.push(LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    });
                    return results;
                }
            }
        }

        // ';' を消費
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);

        results
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
                        let err_idx = self.add_parse_error(
                            &TokenInfo {
                                code_pointer: err_pos,
                            },
                            "array size must be positive",
                        );
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        return None;
                    }
                    *n
                }
                Some((_, token_info)) => {
                    let err_idx = self.add_parse_error(token_info, "expected array size or ']'");
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
                if !name.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
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
                Some((Token::Number(n), _)) if *n > 0 => *n as usize,
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
            base = TypeSpec::Array(Box::new(base), size);
        }

        let mut spec = base;
        for _ in 0..ref_depth {
            spec = TypeSpec::Ref(Box::new(spec));
        }
        Ok(spec)
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
        is_final: bool,
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
                    &TokenInfo {
                        code_pointer: start_pos,
                    },
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
                is_final,
                Some(actual_size),
                None,
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
        is_final: bool,
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
                &TokenInfo {
                    code_pointer: start_pos,
                },
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
                    &TokenInfo {
                        code_pointer: start_pos,
                    },
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
                is_final,
                Some(actual_size),
                None,
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
        is_final: bool,
        type_annot: &Option<TypeSpec>,
        results: &mut Vec<LocatedStatement>,
    ) {
        let mut struct_name = None;
        if let Some(TypeSpec::Named(name)) = type_annot {
            struct_name = Some(name.clone());
        }

        let expr = if let Some(name) = struct_name {
            let mut values = Vec::new();
            loop {
                if let Some((Token::ParenthesisR, _)) = self.iter.peek() {
                    self.iter.next();
                    break;
                }
                let (val, mut errs) = parse_to_expression_tree_root(self.iter);
                self.code_parse_error.append(&mut errs);
                values.push((*val).clone());

                match self.iter.peek() {
                    Some((Token::Comma, _)) => {
                        self.iter.next();
                    }
                    Some((Token::ParenthesisR, _)) => {
                        self.iter.next();
                        break;
                    }
                    Some((_, token_info)) => {
                        let err_idx = self.add_parse_error(
                            token_info,
                            "expected ',' or ')' in struct initializer",
                        );
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        return;
                    }
                    None => {
                        let err_idx = self.add_end_error("unexpected end of input");
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        });
                        return;
                    }
                }
            }

            let end_pos = self.current_pos_or(start_pos);
            let loc = SourceLocation::new(start_pos, end_pos);
            Box::new(LocatedExpression {
                expression: Expression::StructLiteral(name, values),
                location: loc,
            })
        } else {
            let (expr, mut errs) = parse_to_expression_tree_root(self.iter);
            self.code_parse_error.append(&mut errs);
            match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisR);
            expr
        };

        let end_pos = self.current_pos_or(start_pos);
        let loc = SourceLocation::new(start_pos, end_pos);

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
            statement: Statement::VariableDeclaration(
                id.to_string(),
                init_expr,
                is_static,
                is_final,
                None,
                type_annot.clone(),
            ),
            location: loc,
        });
    }

    fn parse_variable_declarations(
        &mut self,
        start_pos: usize,
        is_static: bool,
        is_final: bool,
    ) -> Vec<LocatedStatement> {
        // Keyword トークンがコロンを内包済みのため、ここでのコロン消費は不要

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

            let mut type_annot = None;
            if let Some((Token::At, _)) = self.iter.peek() {
                self.iter.next();
                match self.parse_type_spec() {
                    Ok(spec) => type_annot = Some(spec),
                    Err(e) => {
                        results.push(LocatedStatement {
                            statement: Statement::Invalid(e),
                            location: SourceLocation::from_single(start_pos),
                        });
                        self.skip_to_semicolon();
                        return results;
                    }
                }
            }

            // 配列サイズのチェック
            let (bracket_specified, array_size) = if type_annot.is_none() {
                match self.parse_array_size(start_pos, &mut results) {
                    Some(v) => v,
                    None => {
                        self.skip_to_semicolon();
                        return results;
                    }
                }
            } else {
                (false, None)
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
                            &id,
                            start_pos,
                            array_size,
                            is_static,
                            is_final,
                            &mut results,
                        );
                        if !ok {
                            self.skip_to_semicolon();
                            return results;
                        }
                    } else if let Some((Token::BracketL, _)) = self.iter.peek() {
                        // 数値リスト初期化: ([val1, val2, val3])
                        self.iter.next(); // '[' を消費
                        let ok = self.parse_array_list_init(
                            &id,
                            start_pos,
                            array_size,
                            is_static,
                            is_final,
                            &mut results,
                        );
                        if !ok {
                            self.skip_to_semicolon();
                            return results;
                        }
                    } else {
                        // エラー: 配列初期化には '[' か文字列リテラルが必要
                        let err_idx = self.add_parse_error(
                            &TokenInfo {
                                code_pointer: start_pos,
                            },
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
                    self.parse_variable_init(
                        &id,
                        start_pos,
                        is_static,
                        is_final,
                        &type_annot,
                        &mut results,
                    );
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
                        is_final,
                        array_size,
                        type_annot.clone(),
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

    /// `func:` キーワードに続くエイリアスパラメータリストをパースする。
    ///
    /// 呼び出し元で `func: name(args)` まで消費済み。`)` の直後から読み込む。
    /// `,` が続く限り `alias: kind: name(params)` 形式を繰り返しパースする。
    fn parse_alias_params(&mut self, start_pos: usize) -> Vec<AliasParam> {
        let mut params = Vec::new();
        // ',' alias: kind: name ... という連続パターン
        while matches!(self.iter.peek(), Some((Token::Comma, _))) {
            self.iter.next(); // ',' を消費

            // 次が alias: キーワードでなければ終了（通常関数の場合の安全策）
            if !matches!(self.iter.peek(), Some((Token::Keyword(Keyword::Alias), _))) {
                // comma を戻すことができないので、エラーとして終了
                break;
            }
            self.iter.next(); // 'alias:' キーワードを消費

            // 'func:', 'constexpr:', 'static:' のいずれかを期待
            match self.iter.next() {
                Some((Token::Keyword(Keyword::Func), _)) => {
                    // alias: func: name(arg1, arg2, ...)
                    let param_name = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id)
                    {
                        Ok(x) => x,
                        Err(_) => break,
                    };
                    // 引数リストをパース
                    match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisL);
                    let mut func_args = Vec::new();
                    loop {
                        match self.iter.peek() {
                            Some((Token::ParenthesisR, _)) => {
                                self.iter.next();
                                break;
                            }
                            Some((Token::Identifier(_), _)) => {
                                if let Some((Token::Identifier(arg_name), _)) = self.iter.next() {
                                    func_args.push(arg_name.clone());
                                }
                                if let Some((Token::Comma, _)) = self.iter.peek() {
                                    self.iter.next();
                                }
                            }
                            _ => {
                                self.iter.next();
                                break;
                            }
                        }
                    }
                    params.push(AliasParam {
                        name: param_name.to_string(),
                        kind: AliasParamKind::Func(func_args),
                    });
                }
                Some((Token::Keyword(Keyword::Constexpr), _)) => {
                    // alias: constexpr: name
                    let param_name = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id)
                    {
                        Ok(x) => x,
                        Err(_) => break,
                    };
                    params.push(AliasParam {
                        name: param_name.to_string(),
                        kind: AliasParamKind::Constexpr,
                    });
                }
                Some((Token::Keyword(Keyword::Static), _)) => {
                    // alias: static: name
                    let param_name = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id)
                    {
                        Ok(x) => x,
                        Err(_) => break,
                    };
                    params.push(AliasParam {
                        name: param_name.to_string(),
                        kind: AliasParamKind::Static,
                    });
                }
                Some((_, token_info)) => {
                    self.add_parse_error(
                        token_info,
                        "expected 'func:', 'constexpr:', or 'static:' after 'alias:' in template parameter",
                    );
                    break;
                }
                None => {
                    self.add_end_error("unexpected end of input in template alias parameter");
                    break;
                }
            }
        }
        let _ = start_pos;
        params
    }

    fn parse_struct_field(&mut self, _start_pos: usize) -> Result<StructFieldDecl, usize> {
        let name = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };

        if let Some((Token::At, _)) = self.iter.peek() {
            self.iter.next();
            let type_spec = self.parse_type_spec()?;
            return Ok(StructFieldDecl {
                name: name.to_string(),
                type_spec: Some(type_spec),
                array_size: None,
            });
        }

        if let Some((Token::BracketL, _)) = self.iter.peek() {
            self.iter.next();
            let size = match self.iter.next() {
                Some((Token::Number(n), token_info)) if *n > 0 => *n as usize,
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
            return Ok(StructFieldDecl {
                name: name.to_string(),
                type_spec: None,
                array_size: Some(size),
            });
        }

        Ok(StructFieldDecl {
            name: name.to_string(),
            type_spec: None,
            array_size: None,
        })
    }

    fn parse_to_statements_struct(&mut self, start_pos: usize) -> LocatedStatement {
        self.iter.next();
        let name = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
            Ok(x) => x,
            Err(e) => {
                return LocatedStatement {
                    statement: Statement::Invalid(e),
                    location: SourceLocation::from_single(start_pos),
                }
            }
        };

        if !name.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
            let err_idx = self.add_parse_error(
                &TokenInfo {
                    code_pointer: start_pos,
                },
                "struct name must start with an uppercase letter",
            );
            return LocatedStatement {
                statement: Statement::Invalid(err_idx),
                location: SourceLocation::from_single(start_pos),
            };
        }

        match_expect_token_unused!(self, self.iter.next(), Token::ParenthesisL);
        let mut fields = Vec::new();

        loop {
            match self.iter.peek() {
                Some((Token::ParenthesisR, _)) => {
                    self.iter.next();
                    break;
                }
                Some((Token::Comma, token_info)) => {
                    self.add_parse_error(token_info, "unexpected comma");
                    self.iter.next();
                }
                Some(_) => match self.parse_struct_field(start_pos) {
                    Ok(field) => {
                        fields.push(field);
                        if let Some((Token::Comma, _)) = self.iter.peek() {
                            self.iter.next();
                        }
                    }
                    Err(e) => {
                        return LocatedStatement {
                            statement: Statement::Invalid(e),
                            location: SourceLocation::from_single(start_pos),
                        };
                    }
                },
                None => {
                    let err_idx =
                        self.add_end_error("unexpected end of input in struct declaration");
                    return LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    };
                }
            }
        }

        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        let end_pos = self.current_pos_or(start_pos);
        LocatedStatement {
            statement: Statement::StructDeclaration(name.to_string(), fields),
            location: SourceLocation::new(start_pos, end_pos),
        }
    }

    fn parse_to_statements_namespace(&mut self, start_pos: usize) -> LocatedStatement {
        // 呼び出し元が既に Token::Keyword(Keyword::Namespace) を確認済み
        // Keyword トークンがコロンを内包済みのため、コロン消費は不要
        self.iter.next();
        let name = match match_expect_token!(self, self.iter.next(), Token::Identifier(id) => id) {
            Ok(x) => x,
            Err(e) => {
                return LocatedStatement {
                    statement: Statement::Invalid(e),
                    location: SourceLocation::from_single(start_pos),
                };
            }
        };
        let body = self.parse_to_statements_block();
        let end_pos = self.current_pos_or(start_pos);
        LocatedStatement {
            statement: Statement::NamespaceDeclaration(name.to_string(), body),
            location: SourceLocation::new(start_pos, end_pos),
        }
    }

    fn parse_to_statements_func(&mut self, start_pos: usize) -> LocatedStatement {
        // 呼び出し元が既に Token::Keyword(Keyword::Func) を確認済み
        // Keyword トークンがコロンを内包済みのため、コロン消費は不要
        self.iter.next();
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
        let mut args = Vec::<(String, Option<TypeSpec>)>::new();
        enum State {
            L,
            Var,
            Comma,
        }
        let mut state = State::L;
        loop {
            match self.iter.peek() {
                Some((Token::Identifier(_), _)) => {
                    let (Token::Identifier(name), token_info) = self.iter.next().unwrap() else {
                        unreachable!();
                    };
                    if let State::Var = state {
                        self.add_parse_error(token_info, "expected ','");
                    }
                    let mut type_annot = None;
                    if let Some((Token::At, _)) = self.iter.peek() {
                        self.iter.next();
                        match self.parse_type_spec() {
                            Ok(spec) => type_annot = Some(spec),
                            Err(e) => {
                                return LocatedStatement {
                                    statement: Statement::Invalid(e),
                                    location: SourceLocation::from_single(start_pos),
                                };
                            }
                        }
                    }
                    args.push((name.clone(), type_annot));
                    state = State::Var;
                }
                Some((Token::Comma, token_info)) => {
                    self.iter.next();
                    if let State::Var = state {
                        state = State::Comma;
                    } else {
                        self.add_parse_error(token_info, "unexpected ','");
                    }
                }
                Some((Token::ParenthesisR, token_info)) => {
                    self.iter.next();
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
                Some((token, token_info)) => {
                    self.iter.next();
                    self.add_parse_error(
                        token_info,
                        format!("unexpected token {}", token.describe()),
                    );
                    break;
                }
                None => {
                    self.add_end_error("unexpected end of input");
                    break;
                }
            }
        }

        let mut return_type = None;
        if let Some((Token::At, _)) = self.iter.peek() {
            self.iter.next();
            match self.parse_type_spec() {
                Ok(spec) => return_type = Some(spec),
                Err(e) => {
                    return LocatedStatement {
                        statement: Statement::Invalid(e),
                        location: SourceLocation::from_single(start_pos),
                    };
                }
            }
        }

        // ')' の後に ',' alias: ... が続く場合はテンプレート関数
        if matches!(self.iter.peek(), Some((Token::Comma, _))) {
            // 次が alias: キーワードかどうか先読みして確認できないため、
            // parse_alias_params に委譲する。alias: が続かない場合は空リストを返す。
            let alias_params = self.parse_alias_params(start_pos);
            if !alias_params.is_empty() {
                // テンプレート関数: `{` を期待
                if let Err(e) = match_expect_token!(self, self.iter.peek(), Token::BraceL) {
                    self.iter.next();
                    return LocatedStatement {
                        statement: Statement::Invalid(e),
                        location: SourceLocation::from_single(start_pos),
                    };
                }
                let body = self.parse_to_statements_block();
                let end_pos = self.current_pos_or(start_pos);
                return LocatedStatement {
                    statement: Statement::TemplateFunctionDefinition {
                        name: id.clone(),
                        args,
                        alias_params,
                        body,
                        return_type,
                    },
                    location: SourceLocation::new(start_pos, end_pos),
                };
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
            statement: Statement::FunctionDeclaration(id.clone(), args, body, return_type),
            location: SourceLocation::new(start_pos, end_pos),
        }
    }

    fn parse_to_statements_return(&mut self, start_pos: usize) -> LocatedStatement {
        // 呼び出し元が既に Token::Keyword(Keyword::Return) を確認済み
        // Keyword トークンがコロンを内包済みのため、コロン消費は不要
        // 極源: return:; 形式のみ有効。旧来の return; 構文は廃止。
        self.iter.next();
        // return:; (void return) - セミコロンが次に来たら void return
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

    /// `for:` キーワードを消費して for 文をパースする。
    fn parse_to_statements_for(&mut self, start_pos: usize) -> LocatedStatement {
        self.iter.next(); // 'for' キーワードを消費（コロンも内包済み）
        let init = self.parse_to_statements_block();
        let cond = self.parse_to_statements_block();
        let step = self.parse_to_statements_block();
        let body = self.parse_to_statements_block();
        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
        let end_pos = self.current_pos_or(start_pos);
        LocatedStatement {
            statement: Statement::For(init, cond, step, body),
            location: SourceLocation::new(start_pos, end_pos),
        }
    }

    /// `repeat:` キーワードを消費して repeat 文をパースし、Statement::For に脱糖する。
    fn parse_to_statements_repeat(&mut self, start_pos: usize) -> LocatedStatement {
        self.iter.next(); // 'repeat' キーワードを消費（コロンも内包済み）

        // 最初の式をパース
        let (first_expr, mut first_errors) = parse_to_expression_tree_root(self.iter);
        self.code_parse_error.append(&mut first_errors);

        // 次のトークンによって形式を判定
        // ここで peek() の型は &(Token, TokenInfo)
        let next_is_semi = matches!(self.iter.peek(), Some((Token::Semicolon, _)));
        let next_is_comma = matches!(self.iter.peek(), Some((Token::Comma, _)));

        if next_is_semi {
            // Form 3: repeat: body; → 無限ループ
            self.iter.next(); // ';' を消費
            let end_pos = self.current_pos_or(start_pos);
            LocatedStatement {
                statement: desugar_repeat_form3(first_expr, start_pos),
                location: SourceLocation::new(start_pos, end_pos),
            }
        } else if next_is_comma {
            self.iter.next(); // ',' を消費
                              // 初期化宣言として解釈: first_expr は Expression::Function(name, [init_val]) であるべき
            match first_expr.expression {
                Expression::Function(name, mut args) if args.len() == 1 => {
                    let init_val = args.remove(0);
                    // 2番目の式をパース
                    let (second_expr, mut second_errors) = parse_to_expression_tree_root(self.iter);
                    self.code_parse_error.append(&mut second_errors);

                    let next2_is_semi = matches!(self.iter.peek(), Some((Token::Semicolon, _)));
                    let next2_is_comma = matches!(self.iter.peek(), Some((Token::Comma, _)));

                    if next2_is_semi {
                        // Form 2: repeat: i(init), body; → カウンタ付き無限ループ
                        self.iter.next(); // ';' を消費
                        let end_pos = self.current_pos_or(start_pos);
                        LocatedStatement {
                            statement: desugar_repeat_form2(name, init_val, second_expr, start_pos),
                            location: SourceLocation::new(start_pos, end_pos),
                        }
                    } else if next2_is_comma {
                        // Form 1: repeat: i(init), N, body;
                        self.iter.next(); // ',' を消費
                        let (body_expr, mut body_errors) = parse_to_expression_tree_root(self.iter);
                        self.code_parse_error.append(&mut body_errors);
                        match_expect_token_unused!(self, self.iter.next(), Token::Semicolon);
                        let end_pos = self.current_pos_or(start_pos);
                        LocatedStatement {
                            statement: desugar_repeat_form1(
                                name,
                                init_val,
                                second_expr,
                                body_expr,
                                start_pos,
                            ),
                            location: SourceLocation::new(start_pos, end_pos),
                        }
                    } else {
                        let err_idx = self.add_end_error(
                            "expected ',' or ';' after repeat counter/limit expression",
                        );
                        self.skip_to_semicolon();
                        LocatedStatement {
                            statement: Statement::Invalid(err_idx),
                            location: SourceLocation::from_single(start_pos),
                        }
                    }
                }
                _ => {
                    let err_idx = self.add_parse_error(
                        &TokenInfo {
                            code_pointer: start_pos,
                        },
                        "repeat: expected counter declaration like 'i(0)' before ','",
                    );
                    self.skip_to_semicolon();
                    LocatedStatement {
                        statement: Statement::Invalid(err_idx),
                        location: SourceLocation::from_single(start_pos),
                    }
                }
            }
        } else {
            let err_idx = self.add_end_error("expected ',' or ';' after repeat expression");
            self.skip_to_semicolon();
            LocatedStatement {
                statement: Statement::Invalid(err_idx),
                location: SourceLocation::from_single(start_pos),
            }
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
                Token::Keyword(Keyword::Constexpr) => {
                    statements.extend(self.parse_constexpr_declarations(start_pos));
                    continue;
                }
                Token::Keyword(Keyword::Alias) => {
                    statements.extend(self.parse_alias_declarations(start_pos));
                    continue;
                }
                Token::Keyword(Keyword::Final) => {
                    statements.extend(self.parse_to_statements_final_variable(start_pos));
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
                    self.iter.next(); // while キーワードを消費（コロンも内包済み）

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
                Token::Keyword(Keyword::For) => {
                    let stmt = self.parse_to_statements_for(start_pos);
                    statements.push(stmt);
                    continue;
                }
                Token::Keyword(Keyword::Repeat) => {
                    let stmt = self.parse_to_statements_repeat(start_pos);
                    statements.push(stmt);
                    continue;
                }
                Token::Keyword(Keyword::Namespace) => {
                    let stmt = self.parse_to_statements_namespace(start_pos);
                    statements.push(stmt);
                    continue;
                }
                Token::Keyword(Keyword::Struct) => {
                    let stmt = self.parse_to_statements_struct(start_pos);
                    statements.push(stmt);
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

// ============================================================
// repeat 脱糖ヘルパー
// ============================================================

/// 指定した位置の LocatedExpression を構築するヘルパー
fn make_located_expr_at(expr: Expression, pos: usize) -> Box<LocatedExpression> {
    Box::new(LocatedExpression {
        expression: expr,
        location: SourceLocation::from_single(pos),
    })
}

/// repeat Form 3 の脱糖: `repeat: body;`
/// → `for: {} { 1; } {} { body; };`
fn desugar_repeat_form3(body_expr: Box<LocatedExpression>, pos: usize) -> Statement {
    let loc = SourceLocation::from_single(pos);
    let cond = vec![LocatedStatement {
        statement: Statement::Expression(make_located_expr_at(Expression::Factor(1), pos)),
        location: loc.clone(),
    }];
    let body = vec![LocatedStatement {
        statement: Statement::Expression(body_expr),
        location: loc,
    }];
    Statement::For(vec![], cond, vec![], body)
}

/// repeat Form 2 の脱糖: `repeat: i(init), body;`
/// → `for: { let: i(init); } { 1; } { i += 1; } { body; };`
fn desugar_repeat_form2(
    counter_name: String,
    init_val: Box<LocatedExpression>,
    body_expr: Box<LocatedExpression>,
    pos: usize,
) -> Statement {
    let loc = SourceLocation::from_single(pos);
    // 初期化式は代入式 `counter_name = init_val` として構築（パーサと同様）
    let init_assign = make_located_expr_at(
        Expression::Operation2(
            Operator2::Assign,
            make_located_expr_at(Expression::Variable(counter_name.clone()), pos),
            init_val,
        ),
        pos,
    );
    let init = vec![LocatedStatement {
        statement: Statement::VariableDeclaration(
            counter_name.clone(),
            init_assign,
            false,
            false,
            None,
            None,
        ),
        location: loc.clone(),
    }];
    let cond = vec![LocatedStatement {
        statement: Statement::Expression(make_located_expr_at(Expression::Factor(1), pos)),
        location: loc.clone(),
    }];
    let step_plus = make_located_expr_at(
        Expression::Operation2(
            Operator2::PlusAssign,
            make_located_expr_at(Expression::Variable(counter_name.clone()), pos),
            make_located_expr_at(Expression::Factor(1), pos),
        ),
        pos,
    );
    let step = vec![LocatedStatement {
        statement: Statement::Expression(step_plus),
        location: loc.clone(),
    }];
    let body = vec![LocatedStatement {
        statement: Statement::Expression(body_expr),
        location: loc,
    }];
    Statement::For(init, cond, step, body)
}

/// repeat Form 1 の脱糖: `repeat: i(init), N, body;`
/// → `for: { let: i(init); } { i < N; } { i += 1; } { body; };`
/// i の初期値から N 未満の間ループする（C言語の for(i=init; i<N; i++) と同等）
fn desugar_repeat_form1(
    counter_name: String,
    init_val: Box<LocatedExpression>,
    n_expr: Box<LocatedExpression>,
    body_expr: Box<LocatedExpression>,
    pos: usize,
) -> Statement {
    let loc = SourceLocation::from_single(pos);
    // 初期化式は代入式として構築（パーサと同様）
    let counter_assign = make_located_expr_at(
        Expression::Operation2(
            Operator2::Assign,
            make_located_expr_at(Expression::Variable(counter_name.clone()), pos),
            init_val,
        ),
        pos,
    );
    let init = vec![LocatedStatement {
        statement: Statement::VariableDeclaration(
            counter_name.clone(),
            counter_assign,
            false,
            false,
            None,
            None,
        ),
        location: loc.clone(),
    }];
    // 条件: i < N（n_expr は毎回評価される）
    let cond = vec![LocatedStatement {
        statement: Statement::Expression(make_located_expr_at(
            Expression::Operation2(
                Operator2::Less,
                make_located_expr_at(Expression::Variable(counter_name.clone()), pos),
                n_expr,
            ),
            pos,
        )),
        location: loc.clone(),
    }];
    // ステップ: i += 1
    let step_i_plus = make_located_expr_at(
        Expression::Operation2(
            Operator2::PlusAssign,
            make_located_expr_at(Expression::Variable(counter_name.clone()), pos),
            make_located_expr_at(Expression::Factor(1), pos),
        ),
        pos,
    );
    let step = vec![LocatedStatement {
        statement: Statement::Expression(step_i_plus),
        location: loc.clone(),
    }];
    let body = vec![LocatedStatement {
        statement: Statement::Expression(body_expr),
        location: loc,
    }];
    Statement::For(init, cond, step, body)
}

#[cfg(test)]
mod test;
