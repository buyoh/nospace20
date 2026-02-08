//! # Semantic Analyzer
//!
//! 意味解析器。ASTを実行可能な構造に変換する。
//!
//! 主な責務:
//! - 変数・関数の識別子解決
//! - スコープ構造の構築
//! - 実行可能な中間表現への変換

use std::collections::BTreeMap;

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{Expression, LocatedStatement, Operator1, Operator2, Statement},
};

/// 解決済み識別子への参照
///
/// Phase 2 で導入。変数・関数の識別子を文字列ではなく、
/// スコープ階層とローカルインデックスで管理することで、
/// 実行時の文字列検索を排除し、O(1) アクセスを実現する。
///
/// Phase 3 で is_global フラグを追加。グローバル変数は Environment に保持されるため、
/// ローカル変数とは別の参照方法が必要。
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRef {
    /// スコープの深さ（0 = 現在のスコープ、1 = 親スコープ、...）
    pub scope_depth: usize,
    /// スコープ内でのインデックス
    pub local_index: usize,
    /// Phase 3: グローバル変数かどうか
    /// true の場合、Environment.global_variables でアクセス
    /// false の場合、LocalEnvironment.scope_stack でアクセス
    pub is_global: bool,
}

struct IdentifierInfo {
    // name: String,
    idx: usize, // TODO: more safety
}

enum Identifier {
    Function(IdentifierInfo),
    Variable(IdentifierInfo),
}

/// 変数情報
///
/// Phase 3 で is_static フラグを追加。static 変数は関数スコープ境界を越えてアクセス可能。
#[derive(Clone)]
pub(crate) struct Variable {
    // NOTE: ここに初期化情報は置かない
    pub identifier: String, // TODO: use IdentifierInfo
    /// Phase 3: static フラグ
    /// true の場合、親の関数スコープからもアクセス可能
    /// グローバル変数は暗黙的に is_static = true
    pub is_static: bool,
}

/// ブロック（文の列とスコープ情報）
pub(crate) struct Block {
    pub scope: Scope,
    pub statements: Vec<ExecStatement>,
}

/// 実行可能な式を表す。
///
/// `Expression` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない (パース成功後のみ生成される)
/// - スコープ解決済みの識別子情報を保持する (Phase 2 で実装)
///   変数は IdentifierRef を使用することで、実行時の文字列検索を排除し、O(1) アクセスを実現。
///   関数は現状組み込み関数のみのため、文字列のまま保持。
// #[derive(Clone)] // TODO: REMOVE
pub(crate) enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    /// 関数呼び出し
    /// Phase 2: 組み込み関数のみのため String のまま（ユーザー定義関数は Phase 3 以降）
    Function(String, Vec<Box<ExecExpression>>),
    Factor(i64),
    /// 変数参照
    /// Phase 2: String から IdentifierRef に変更
    Variable(IdentifierRef),
}

/// 実行可能な文を表す。
///
/// `Statement` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない
/// - 宣言文 (VariableDeclaration, FunctionDeclaration) を持たない
///   (宣言は `Scope` 構造に変換される)
// #[derive(Clone)] // TODO: REMOVE
pub(crate) enum ExecStatement {
    Return(Box<ExecExpression>),
    Break,
    Continue,
    Expression(Box<ExecExpression>),
}

/// 式を ExecExpression に変換する（識別子解決あり）
///
/// Phase 2 で導入。ScopeResolver を使用して変数名・関数名を IdentifierRef に解決する。
fn convert_to_exec_expression_with_resolver(
    expr: &Box<Expression>,
    parent_resolver: &ScopeResolver,
) -> Result<Box<ExecExpression>, Vec<CodeParseError>> {
    match expr.as_ref() {
        Expression::Operation1(Operator1::Ref, inner) => {
            // & は変数に対してのみ使用可能
            match inner.as_ref() {
                Expression::Variable(name) => {
                    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                        vec![code_parse_error!(format!("undefined variable: {}", name))]
                    })?;
                    Ok(Box::new(ExecExpression::Operation1(
                        Operator1::Ref,
                        Box::new(ExecExpression::Variable(id_ref)),
                    )))
                }
                _ => Err(vec![code_parse_error!(
                    "reference operator (&) can only be applied to variables"
                )]),
            }
        }
        Expression::Operation1(op, x) => Ok(Box::new(ExecExpression::Operation1(
            op.to_owned(),
            convert_to_exec_expression_with_resolver(&x, parent_resolver)?,
        ))),
        Expression::Operation2(op, l, r) => Ok(Box::new(ExecExpression::Operation2(
            op.to_owned(),
            convert_to_exec_expression_with_resolver(&l, parent_resolver)?,
            convert_to_exec_expression_with_resolver(&r, parent_resolver)?,
        ))),
        Expression::If(cond, stat1, stat2) => {
            let (s1, es1) = analyze_internal_with_parent(
                stat1,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
            )?;
            let (s2, es2) = analyze_internal_with_parent(
                stat2,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
            )?;
            Ok(Box::new(ExecExpression::If(
                convert_to_exec_expression_with_resolver(cond, parent_resolver)?,
                Block {
                    scope: s1.build(false, Vec::new()), // ブロックは関数スコープではなく、root_statementsは空
                    statements: es1,
                },
                Block {
                    scope: s2.build(false, Vec::new()), // ブロックは関数スコープではなく、root_statementsは空
                    statements: es2,
                },
            )))
        }
        Expression::While(expr, stat) => {
            let (s, es) = analyze_internal_with_parent(
                stat,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
            )?;
            Ok(Box::new(ExecExpression::While(
                convert_to_exec_expression_with_resolver(expr, parent_resolver)?,
                Block {
                    scope: s.build(false, Vec::new()), // ブロックは関数スコープではなく、root_statementsは空
                    statements: es,
                },
            )))
        }
        Expression::Function(f, a) => {
            // Phase 2: 関数は組み込み関数のみなので文字列のまま保持
            // ユーザー定義関数は Phase 3 以降で対応
            let mut args = Vec::new();
            for e in a {
                args.push(convert_to_exec_expression_with_resolver(
                    e,
                    parent_resolver,
                )?);
            }
            Ok(Box::new(ExecExpression::Function(f.clone(), args)))
        }
        Expression::Factor(v) => Ok(Box::new(ExecExpression::Factor(v.to_owned()))),
        Expression::Variable(v) => {
            // 変数名を解決
            let var_ref = parent_resolver
                .resolve_variable(v)
                .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", v))])?;
            Ok(Box::new(ExecExpression::Variable(var_ref)))
        }
        // パースエラー時のみ Invalid が生成されるため、正常系では到達しない
        Expression::Invalid(_) => {
            unreachable!("Expression::Invalid should not reach semantic analysis")
        }
    }
}

fn convert_to_exec_expression(
    expr: &Box<Expression>,
) -> Result<Box<ExecExpression>, Vec<CodeParseError>> {
    // Phase 2: 後方互換性のため残すが、内部的には resolver を使用
    // TODO: この関数は削除予定（全ての呼び出しを convert_to_exec_expression_with_resolver に置き換える）
    let resolver = ScopeResolver::new();
    convert_to_exec_expression_with_resolver(expr, &resolver)
}

pub(crate) struct Function {
    pub args: Vec<String>, // TODO: change string to identifier_ptr
    /// 事前計算された引数のインデックス（Phase 2 最適化）
    /// 関数呼び出し時の引数初期化を O(args) にするため、
    /// 各引数の block.scope 内でのインデックスを保持
    pub arg_indices: Vec<usize>,
    pub block: Block,
    // pub identifier: String,
}

/// スコープ情報
///
/// Phase 2 で変数インデックス管理を追加。
/// 変数名からローカルインデックスへのマッピングを保持することで、
/// 実行時に Vec<i64> ベースの高速アクセスを可能にする。
///
/// Phase 3 で is_function_scope フラグを追加。関数スコープ境界を越える場合、
/// static 変数のみアクセス可能。
///
/// Phase 3 でルートスコープに実行文（グローバル変数の初期化）を追加。
pub struct Scope {
    identifier_map: BTreeMap<String, Identifier>,

    /// 変数名からローカルインデックスへのマップ
    /// Phase 2 で追加: 識別子解決時に使用
    pub(crate) variable_indices: BTreeMap<String, usize>,

    pub(crate) variables: Vec<Variable>,

    /// 変数の総数
    /// Phase 2 で追加: インタプリタが Vec<i64> を初期化する際に使用
    pub(crate) variable_count: usize,

    functions: Vec<Function>,

    /// Phase 3: このスコープが関数スコープかどうか
    /// true の場合、非 static 変数は親スコープからアクセス不可
    /// Root スコープと Function スコープで true
    pub(crate) is_function_scope: bool,

    /// Phase 3: ルートスコープの実行文（グローバル変数の初期化）
    /// 関数スコープ・ブロックスコープでは空
    pub(crate) root_statements: Vec<ExecStatement>,
}

impl Scope {
    pub(crate) fn get_function(&self, id: &str) -> Option<&Function> {
        if let Some(Identifier::Function(info)) = self.identifier_map.get(id) {
            Some(&self.functions[info.idx])
        } else {
            None
        }
    }

    pub(crate) fn get_variable(&self, id: &str) -> Option<&Variable> {
        if let Some(Identifier::Variable(info)) = self.identifier_map.get(id) {
            Some(&self.variables[info.idx])
        } else {
            None
        }
    }
}

enum ScopeType {
    Root,
    Function,
    Block,
}

/// スコープ情報（ScopeResolver 用）
///
/// Phase 3 で追加。関数境界チェックのため、各スコープの追加情報を保持する。
#[derive(Clone)]
struct ScopeInfo<'a> {
    /// 変数名からインデックスへのマップ
    var_indices: &'a BTreeMap<String, usize>,
    /// 変数情報（static フラグ確認用）
    variables: &'a Vec<Variable>,
    /// このスコープが関数スコープかどうか
    is_function_scope: bool,
}

/// スコープ解決のためのコンテキスト
///
/// Phase 2 で導入。2パス解析のパス2で使用され、
/// 変数名・関数名を IdentifierRef に解決する。
///
/// Phase 3 で関数境界チェックを追加。親の関数スコープの非 static 変数には
/// アクセスできないようにする。
struct ScopeResolver<'a> {
    /// スコープスタック（末尾が現在のスコープ）
    /// Phase 3: スコープ情報を保持するように変更
    scope_stack: Vec<ScopeInfo<'a>>,
}

impl<'a> ScopeResolver<'a> {
    fn new() -> Self {
        Self {
            scope_stack: Vec::new(),
        }
    }

    fn enter_scope(
        &mut self,
        var_indices: &'a BTreeMap<String, usize>,
        variables: &'a Vec<Variable>,
        is_function_scope: bool,
    ) {
        self.scope_stack.push(ScopeInfo {
            var_indices,
            variables,
            is_function_scope,
        });
    }

    fn leave_scope(&mut self) {
        self.scope_stack.pop();
    }

    /// 変数名を解決し、IdentifierRef を返す
    ///
    /// スコープスタックを逆順に探索し、最も近いスコープの変数を見つける。
    /// 関数スコープ境界を越えた場合、static 変数のみアクセス可能。
    /// 見つからない場合は None を返す。
    fn resolve_variable(&self, name: &str) -> Option<IdentifierRef> {
        // 最初に見つけた関数スコープ（自分の関数）より外側の関数スコープを越えた場合、境界を越えたとする
        let mut first_function_scope_depth: Option<usize> = None;

        for (depth, scope_info) in self.scope_stack.iter().rev().enumerate() {
            // 最初の関数スコープを記録
            if scope_info.is_function_scope && first_function_scope_depth.is_none() {
                first_function_scope_depth = Some(depth);
            }

            if let Some(&local_index) = scope_info.var_indices.get(name) {
                // 関数境界を越えたかチェック
                // first_function_scope_depth より外側（depth が大きい）の関数スコープに変数がある場合
                let crossed_function_boundary =
                    if let Some(first_func_depth) = first_function_scope_depth {
                        depth > first_func_depth && scope_info.is_function_scope
                    } else {
                        // まだ関数スコープに入っていない（グローバルスコープのみ探索中）
                        false
                    };

                // 関数境界を越えた場合、static 変数のみアクセス可能
                if crossed_function_boundary && !scope_info.variables[local_index].is_static {
                    // 非 static 変数はスキップして探索継続
                    continue;
                }

                // グローバル変数かどうかを判定
                // スタックの最下層（depth == scope_stack.len() - 1）がルートスコープ
                let is_global = depth == self.scope_stack.len() - 1
                    && self
                        .scope_stack
                        .first()
                        .map(|s| s.is_function_scope)
                        .unwrap_or(false);

                return Some(IdentifierRef {
                    scope_depth: depth,
                    local_index,
                    is_global,
                });
            }
        }
        None
    }
}

struct ScopeBuilder {
    identifier_map: BTreeMap<String, Identifier>,
    variables: Vec<Variable>,
    functions: Vec<Function>,
}

impl ScopeBuilder {
    fn new() -> Self {
        Self {
            identifier_map: BTreeMap::new(),
            variables: vec![],
            functions: vec![],
        }
    }

    fn build(self, is_function_scope: bool, root_statements: Vec<ExecStatement>) -> Scope {
        // 変数名からインデックスへのマッピングを構築
        let mut variable_indices = BTreeMap::new();
        for (idx, var) in self.variables.iter().enumerate() {
            variable_indices.insert(var.identifier.clone(), idx);
        }

        let variable_count = self.variables.len();

        Scope {
            identifier_map: self.identifier_map,
            variable_indices,
            variables: self.variables,
            variable_count,
            functions: self.functions,
            is_function_scope,
            root_statements,
        }
    }

    fn add_identifier(
        &mut self,
        name: &str,
        identifier: Identifier,
    ) -> Result<(), Vec<CodeParseError>> {
        if self.identifier_map.contains_key(name) {
            return Err(vec![code_parse_error!(format!(
                "semantic error: the name '{}' is already used",
                name
            ))]);
        }
        self.identifier_map.insert(name.to_string(), identifier);
        Ok(())
    }

    fn add_variable(&mut self, name: &str, var: Variable) -> Result<(), Vec<CodeParseError>> {
        let vi = self.variables.len();
        self.variables.push(var);
        self.add_identifier(name, Identifier::Variable(IdentifierInfo { idx: vi }))
    }

    fn add_function(&mut self, name: &str, func: Function) -> Result<(), Vec<CodeParseError>> {
        let fi = self.functions.len();
        self.functions.push(func);
        self.add_identifier(name, Identifier::Function(IdentifierInfo { idx: fi }))
    }
}

fn analyze_internal(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    analyze_internal_with_parent(statements, scope_type, Vec::new(), None)
}

/// 初期変数と親のresolverを指定して解析する
fn analyze_internal_with_parent(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    initial_vars: Vec<String>,
    parent_resolver: Option<&ScopeResolver>,
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    let mut scope = ScopeBuilder::new();

    // Phase 3: グローバル変数は暗黙的に static
    let is_static = matches!(scope_type, ScopeType::Root);
    let is_function_scope = matches!(scope_type, ScopeType::Root | ScopeType::Function);

    // 初期変数を登録（関数の引数など）
    for var_name in initial_vars {
        scope.add_variable(
            &var_name,
            Variable {
                identifier: var_name.clone(),
                is_static: false, // 関数引数は非 static
            },
        )?;
    }

    // Phase 2: 2パス解析（変数のみ）
    // パス1: 変数宣言収集（ホイスティング対応）
    for located_stat in statements {
        let stat = &located_stat.statement;
        match stat {
            Statement::VariableDeclaration(name, _, is_static_explicit) => {
                // Phase 3: グローバル変数は暗黙的に static
                // Phase 4: 明示的 static も考慮
                let final_is_static = *is_static_explicit || is_static;
                scope.add_variable(
                    name,
                    Variable {
                        identifier: name.clone(),
                        is_static: final_is_static,
                    },
                )?;
            }
            Statement::FunctionDeclaration(_name, _, _) => {
                if !matches!(scope_type, ScopeType::Root) {
                    return Err(vec![code_parse_error!(
                        located_stat.location.start,
                        "semantic error: nested function declaration is not supported"
                    )]);
                }
                // 関数宣言はパス2で処理（ルートスコープのみで、ホイスティング不要）
            }
            _ => {}
        }
    }

    // 変数名からインデックスへのマッピングを先に構築（resolver で使用）
    let mut variable_indices_temp = BTreeMap::new();
    for (idx, var) in scope.variables.iter().enumerate() {
        variable_indices_temp.insert(var.identifier.clone(), idx);
    }

    // Variable を Clone するための一時保存（resolver が参照するため）
    // scope.variables をそのまま使用するのではなく、Scope にまとめて後で参照
    // 一旦 temporary_scope を作って参照を保持
    let temporary_scope = Scope {
        identifier_map: BTreeMap::new(), // 未使用
        variable_indices: variable_indices_temp.clone(),
        variables: scope.variables.clone(), // Clone が必要
        variable_count: scope.variables.len(),
        functions: Vec::new(), // 未使用
        is_function_scope,
        root_statements: Vec::new(), // 未使用
    };

    // 親のresolverを継承して新しいresolverを作成
    let mut resolver = if let Some(parent) = parent_resolver {
        let mut new_resolver = ScopeResolver {
            scope_stack: parent.scope_stack.clone(),
        };
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variables,
            is_function_scope,
        );
        new_resolver
    } else {
        let mut new_resolver = ScopeResolver::new();
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variables,
            is_function_scope,
        );
        new_resolver
    };

    // パス2: 文の変換（識別子解決を伴う）
    let mut exec_statements = Vec::<ExecStatement>::new();
    for located_stat in statements {
        let stat = &located_stat.statement;
        let loc = &located_stat.location;
        match stat {
            Statement::VariableDeclaration(_, init, _) => {
                // 初期化式を変換（変数宣言自体はパス1で完了）
                exec_statements.push(ExecStatement::Expression(
                    convert_to_exec_expression_with_resolver(init, &resolver)?,
                ));
            }
            Statement::FunctionDeclaration(name, args, block) => {
                // Phase 3: 関数本体を解析（親resolverを渡してグローバル変数を参照可能にする）
                let (s, es) = analyze_internal_with_parent(
                    block,
                    ScopeType::Function,
                    args.clone(),
                    Some(&resolver),
                )?;
                let built_scope = s.build(true, Vec::new()); // 関数スコープ、root_statementsは空

                // 引数のインデックスを事前計算（Phase 2 最適化）
                let arg_indices: Vec<usize> = args
                    .iter()
                    .map(|arg_name| {
                        *built_scope
                            .variable_indices
                            .get(arg_name)
                            .expect("argument must be registered as variable")
                    })
                    .collect();

                // 関数を登録
                let func = Function {
                    args: args.clone(),
                    arg_indices,
                    block: Block {
                        scope: built_scope,
                        statements: es,
                    },
                };
                scope.add_function(name, func)?;
            }
            Statement::Return(e) => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: return statement outside of function"
                    )]);
                }
                exec_statements.push(ExecStatement::Return(
                    convert_to_exec_expression_with_resolver(e, &resolver)?,
                ));
            }
            Statement::Expression(e) => {
                // Phase 3: ルートスコープでも式文を許可（グローバル変数の初期化式）
                exec_statements.push(ExecStatement::Expression(
                    convert_to_exec_expression_with_resolver(e, &resolver)?,
                ));
            }
            Statement::Continue => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: continue statement outside of function"
                    )]);
                }
                exec_statements.push(ExecStatement::Continue);
            }
            Statement::Break => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: break statement outside of function"
                    )]);
                }
                exec_statements.push(ExecStatement::Break);
            }
            Statement::Invalid(_) => (),
        }
    }

    resolver.leave_scope();
    Ok((scope, exec_statements))
}

pub fn analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    // Phase 3: ルートの実行文（グローバル変数の初期化）も返す
    analyze_internal(root, ScopeType::Root).map(|(scope, root_stmts)| scope.build(true, root_stmts))
    // TODO: validate identifiers
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::SourceLocation;

    #[test]
    fn test_error_return_outside_function() {
        // return:0; at root level should error with position
        let statements = vec![LocatedStatement {
            statement: Statement::Return(Box::new(Expression::Factor(0))),
            location: SourceLocation::new(10, 20),
        }];

        let result = analyze(&statements);
        assert!(result.is_err());

        let errors = match result {
            Err(e) => e,
            Ok(_) => panic!("Expected error"),
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code_pointer, Some(10));
        assert!(errors[0]
            .message
            .contains("return statement outside of function"));
    }

    #[test]
    fn test_error_break_outside_function() {
        let statements = vec![LocatedStatement {
            statement: Statement::Break,
            location: SourceLocation::new(25, 30),
        }];

        let result = analyze(&statements);
        assert!(result.is_err());

        let errors = match result {
            Err(e) => e,
            Ok(_) => panic!("Expected error"),
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code_pointer, Some(25));
        assert!(errors[0]
            .message
            .contains("break statement outside of function"));
    }

    #[test]
    fn test_error_continue_outside_function() {
        let statements = vec![LocatedStatement {
            statement: Statement::Continue,
            location: SourceLocation::new(35, 45),
        }];

        let result = analyze(&statements);
        assert!(result.is_err());

        let errors = match result {
            Err(e) => e,
            Ok(_) => panic!("Expected error"),
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code_pointer, Some(35));
        assert!(errors[0]
            .message
            .contains("continue statement outside of function"));
    }

    #[test]
    fn test_success_expression_at_root_level() {
        // Phase 3: グローバル変数の初期化式を許可
        let statements = vec![LocatedStatement {
            statement: Statement::Expression(Box::new(Expression::Factor(42))),
            location: SourceLocation::new(50, 55),
        }];

        let result = analyze(&statements);
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_nested_function_declaration() {
        // func: outer() { func: inner() {} }
        let inner_func = LocatedStatement {
            statement: Statement::FunctionDeclaration(
                "inner".to_string(),
                vec![],
                vec![], // empty body
            ),
            location: SourceLocation::new(100, 120),
        };

        let outer_func = LocatedStatement {
            statement: Statement::FunctionDeclaration(
                "outer".to_string(),
                vec![],
                vec![inner_func],
            ),
            location: SourceLocation::new(80, 130),
        };

        let statements = vec![outer_func];
        let result = analyze(&statements);
        assert!(result.is_err());

        let errors = match result {
            Err(e) => e,
            Ok(_) => panic!("Expected error"),
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code_pointer, Some(100));
        assert!(errors[0]
            .message
            .contains("nested function declaration is not supported"));
    }

    #[test]
    fn test_success_block_scoped_variable() {
        let var_decl = LocatedStatement {
            statement: Statement::VariableDeclaration(
                "x".to_string(),
                Box::new(Expression::Factor(0)),
                false, // non-static
            ),
            location: SourceLocation::new(150, 160),
        };

        // ブロックスコープでの変数宣言をシミュレート
        // If式のthen節内で変数宣言を試みる
        let if_expr = LocatedStatement {
            statement: Statement::Expression(Box::new(Expression::If(
                Box::new(Expression::Factor(1)),
                vec![var_decl], // block内の変数宣言
                vec![],
            ))),
            location: SourceLocation::new(140, 170),
        };

        let func = LocatedStatement {
            statement: Statement::FunctionDeclaration("test".to_string(), vec![], vec![if_expr]),
            location: SourceLocation::new(135, 175),
        };

        let statements = vec![func];
        let result = analyze(&statements);
        assert!(result.is_ok());
    }

    #[test]
    fn test_success_global_variable() {
        // Phase 3: グローバル変数を許可
        let var_decl = LocatedStatement {
            statement: Statement::VariableDeclaration(
                "global".to_string(),
                Box::new(Expression::Factor(42)),
                false, // non-static explicitly, but global is implicitly static
            ),
            location: SourceLocation::new(200, 210),
        };

        let statements = vec![var_decl];
        let result = analyze(&statements);
        assert!(result.is_ok());

        let scope = result.unwrap();
        // グローバル変数が登録されていることを確認
        assert_eq!(scope.variable_count, 1);
        assert!(scope.variables[0].is_static); // 暗黙的に static
    }

    #[test]
    fn test_success_simple_function() {
        // func: main() { return:0; }
        let return_stmt = LocatedStatement {
            statement: Statement::Return(Box::new(Expression::Factor(0))),
            location: SourceLocation::new(20, 30),
        };

        let func = LocatedStatement {
            statement: Statement::FunctionDeclaration(
                "main".to_string(),
                vec![],
                vec![return_stmt],
            ),
            location: SourceLocation::new(0, 35),
        };

        let statements = vec![func];
        let result = analyze(&statements);
        assert!(result.is_ok());
    }

    #[test]
    fn test_success_ref_variable() {
        // func: main() { let: x; &x; return:0; }
        let var_decl = LocatedStatement {
            statement: Statement::VariableDeclaration(
                "x".to_string(),
                Box::new(Expression::Factor(0)),
                false,
            ),
            location: SourceLocation::new(20, 30),
        };

        let ref_expr = LocatedStatement {
            statement: Statement::Expression(Box::new(Expression::Operation1(
                Operator1::Ref,
                Box::new(Expression::Variable("x".to_string())),
            ))),
            location: SourceLocation::new(35, 40),
        };

        let return_stmt = LocatedStatement {
            statement: Statement::Return(Box::new(Expression::Factor(0))),
            location: SourceLocation::new(45, 55),
        };

        let func = LocatedStatement {
            statement: Statement::FunctionDeclaration(
                "main".to_string(),
                vec![],
                vec![var_decl, ref_expr, return_stmt],
            ),
            location: SourceLocation::new(0, 60),
        };

        let statements = vec![func];
        let result = analyze(&statements);
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_ref_literal() {
        // func: main() { &5; return:0; }
        let ref_expr = LocatedStatement {
            statement: Statement::Expression(Box::new(Expression::Operation1(
                Operator1::Ref,
                Box::new(Expression::Factor(5)),
            ))),
            location: SourceLocation::new(20, 25),
        };

        let return_stmt = LocatedStatement {
            statement: Statement::Return(Box::new(Expression::Factor(0))),
            location: SourceLocation::new(30, 40),
        };

        let func = LocatedStatement {
            statement: Statement::FunctionDeclaration(
                "main".to_string(),
                vec![],
                vec![ref_expr, return_stmt],
            ),
            location: SourceLocation::new(0, 45),
        };

        let statements = vec![func];
        let result = analyze(&statements);
        assert!(result.is_err());

        let errors = match result {
            Err(e) => e,
            Ok(_) => panic!("Expected error"),
        };
        assert_eq!(errors.len(), 1);
        assert!(errors[0]
            .message
            .contains("reference operator (&) can only be applied to variables"));
    }

    #[test]
    fn test_error_ref_expression() {
        // func: main() { let: x; &(x + 1); return:0; }
        let var_decl = LocatedStatement {
            statement: Statement::VariableDeclaration(
                "x".to_string(),
                Box::new(Expression::Factor(0)),
                false,
            ),
            location: SourceLocation::new(20, 30),
        };

        let ref_expr = LocatedStatement {
            statement: Statement::Expression(Box::new(Expression::Operation1(
                Operator1::Ref,
                Box::new(Expression::Operation2(
                    Operator2::Plus,
                    Box::new(Expression::Variable("x".to_string())),
                    Box::new(Expression::Factor(1)),
                )),
            ))),
            location: SourceLocation::new(35, 45),
        };

        let return_stmt = LocatedStatement {
            statement: Statement::Return(Box::new(Expression::Factor(0))),
            location: SourceLocation::new(50, 60),
        };

        let func = LocatedStatement {
            statement: Statement::FunctionDeclaration(
                "main".to_string(),
                vec![],
                vec![var_decl, ref_expr, return_stmt],
            ),
            location: SourceLocation::new(0, 65),
        };

        let statements = vec![func];
        let result = analyze(&statements);
        assert!(result.is_err());

        let errors = match result {
            Err(e) => e,
            Ok(_) => panic!("Expected error"),
        };
        assert_eq!(errors.len(), 1);
        assert!(errors[0]
            .message
            .contains("reference operator (&) can only be applied to variables"));
    }

    #[test]
    fn test_success_deref_variable() {
        // func: main() { let: p; *p; return:0; }
        let var_decl = LocatedStatement {
            statement: Statement::VariableDeclaration(
                "p".to_string(),
                Box::new(Expression::Factor(0)),
                false,
            ),
            location: SourceLocation::new(20, 30),
        };

        let deref_expr = LocatedStatement {
            statement: Statement::Expression(Box::new(Expression::Operation1(
                Operator1::Deref,
                Box::new(Expression::Variable("p".to_string())),
            ))),
            location: SourceLocation::new(35, 40),
        };

        let return_stmt = LocatedStatement {
            statement: Statement::Return(Box::new(Expression::Factor(0))),
            location: SourceLocation::new(45, 55),
        };

        let func = LocatedStatement {
            statement: Statement::FunctionDeclaration(
                "main".to_string(),
                vec![],
                vec![var_decl, deref_expr, return_stmt],
            ),
            location: SourceLocation::new(0, 60),
        };

        let statements = vec![func];
        let result = analyze(&statements);
        assert!(result.is_ok());
    }
}
