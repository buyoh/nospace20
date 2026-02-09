//! # Semantic Analyzer
//!
//! 意味解析器。ASTを実行可能な構造に変換する。
//!
//! 主な責務:
//! - 変数・関数の識別子解決
//! - スコープ構造の構築
//! - 実行可能な中間表現への変換

mod types;

use std::collections::BTreeMap;

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{Expression, LocatedStatement, Operator1, Operator2, Statement},
};

pub use types::IdentifierRef;
pub(crate) use types::{Block, ExecExpression, ExecStatement, Variable};

struct IdentifierInfo {
    // name: String,
    idx: usize, // TODO: more safety
}

enum Identifier {
    Function(IdentifierInfo),
    Variable(IdentifierInfo),
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
            // & は変数または配列要素に対してのみ使用可能
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
                Expression::ArrayAccess(name, index_expr) => {
                    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                        vec![code_parse_error!(format!("undefined variable: {}", name))]
                    })?;

                    // 配列変数であることを確認
                    let array_size = parent_resolver
                        .get_array_size(name)
                        .ok_or_else(|| {
                            vec![code_parse_error!(format!("undefined variable: {}", name))]
                        })?
                        .ok_or_else(|| {
                            vec![code_parse_error!(format!("'{}' is not an array", name))]
                        })?;

                    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;

                    Ok(Box::new(ExecExpression::Operation1(
                        Operator1::Ref,
                        Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)),
                    )))
                }
                _ => Err(vec![code_parse_error!(
                    "reference operator (&) can only be applied to variables or array elements"
                )]),
            }
        }
        Expression::Operation1(op, x) => Ok(Box::new(ExecExpression::Operation1(
            op.to_owned(),
            convert_to_exec_expression_with_resolver(&x, parent_resolver)?,
        ))),
        Expression::Operation2(op, l, r) => {
            // 複合代入演算子 (+=, -=, *=, /=, %=) を a = a + b の形式に展開
            let (actual_op, actual_l, actual_r) = match op {
                Operator2::PlusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Plus, l.clone(), r.clone())),
                ),
                Operator2::MinusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Minus, l.clone(), r.clone())),
                ),
                Operator2::MultiplyAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Multiply, l.clone(), r.clone())),
                ),
                Operator2::DivideAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Divide, l.clone(), r.clone())),
                ),
                Operator2::ModuloAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Modulo, l.clone(), r.clone())),
                ),
                _ => (op.to_owned(), l, r),
            };
            
            Ok(Box::new(ExecExpression::Operation2(
                actual_op,
                convert_to_exec_expression_with_resolver(&actual_l, parent_resolver)?,
                convert_to_exec_expression_with_resolver(&actual_r, parent_resolver)?,
            )))
        }
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
        Expression::ArrayAccess(name, index_expr) => {
            let id_ref = parent_resolver
                .resolve_variable(name)
                .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?;

            // 配列変数であることを確認
            let array_size = parent_resolver
                .get_array_size(name)
                .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?
                .ok_or_else(|| vec![code_parse_error!(format!("'{}' is not an array", name))])?;

            let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;

            Ok(Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)))
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

    /// 変数名からスロットインデックスへのマップ
    /// Phase 2 で追加: 識別子解決時に使用
    /// 配列の場合、配列の開始スロットインデックスを指す
    pub(crate) variable_indices: BTreeMap<String, usize>,

    /// 変数名から variables ベクタのインデックスへのマップ
    /// 配列対応のため追加: 配列情報を取得する際に使用
    pub(crate) variable_name_to_var_index: BTreeMap<String, usize>,

    pub(crate) variables: Vec<Variable>,

    /// 変数のスロット総数（配列サイズを考慮）
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
    
    /// 指定した名前の関数が存在するかチェックする
    pub fn has_function(&self, id: &str) -> bool {
        self.get_function(id).is_some()
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
    /// 変数名からスロットインデックスへのマップ
    var_indices: &'a BTreeMap<String, usize>,
    /// 変数名から variables ベクタのインデックスへのマップ
    var_name_to_var_index: &'a BTreeMap<String, usize>,
    /// 変数情報（static フラグ、配列サイズ確認用）
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
        var_name_to_var_index: &'a BTreeMap<String, usize>,
        variables: &'a Vec<Variable>,
        is_function_scope: bool,
    ) {
        self.scope_stack.push(ScopeInfo {
            var_indices,
            var_name_to_var_index,
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
                // 変数情報を取得（var_name_to_var_index 経由）
                let var_idx = scope_info.var_name_to_var_index.get(name)?;
                let var = &scope_info.variables[*var_idx];

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
                if crossed_function_boundary && !var.is_static {
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

    /// 変数の配列サイズを取得
    ///
    /// None の場合、変数が見つからない
    /// Some(None) の場合、通常変数（配列ではない）
    /// Some(Some(n)) の場合、サイズ n の配列
    fn get_array_size(&self, name: &str) -> Option<Option<usize>> {
        for scope_info in self.scope_stack.iter().rev() {
            if let Some(&var_idx) = scope_info.var_name_to_var_index.get(name) {
                return Some(scope_info.variables[var_idx].array_size);
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
        // 変数名からスロットインデックスへのマッピングを構築
        // 配列の場合、変数の開始スロットインデックスを記録
        let mut variable_indices = BTreeMap::new();
        let mut variable_name_to_var_index = BTreeMap::new();
        let mut slot_index = 0;
        for (var_idx, var) in self.variables.iter().enumerate() {
            variable_indices.insert(var.identifier.clone(), slot_index);
            variable_name_to_var_index.insert(var.identifier.clone(), var_idx);
            slot_index += var.array_size.unwrap_or(1);
        }
        let variable_count = slot_index;

        Scope {
            identifier_map: self.identifier_map,
            variable_indices,
            variable_name_to_var_index,
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
                array_size: None, // 関数引数は配列ではない
            },
        )?;
    }

    // Phase 2: 2パス解析（変数のみ）
    // パス1: 変数宣言収集（ホイスティング対応）
    for located_stat in statements {
        let stat = &located_stat.statement;
        match stat {
            Statement::VariableDeclaration(name, _, is_static_explicit, array_size) => {
                // Phase 3: グローバル変数は暗黙的に static
                // Phase 4: 明示的 static も考慮
                let final_is_static = *is_static_explicit || is_static;
                scope.add_variable(
                    name,
                    Variable {
                        identifier: name.clone(),
                        is_static: final_is_static,
                        array_size: array_size.map(|n| n as usize),
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
    // 配列サイズを考慮したスロットインデックスを使用
    let mut variable_indices_temp = BTreeMap::new();
    let mut variable_name_to_var_index_temp = BTreeMap::new();
    let mut slot_index = 0;
    for (idx, var) in scope.variables.iter().enumerate() {
        variable_indices_temp.insert(var.identifier.clone(), slot_index);
        variable_name_to_var_index_temp.insert(var.identifier.clone(), idx);
        slot_index += var.array_size.unwrap_or(1);
    }

    // Variable を Clone するための一時保存（resolver が参照するため）
    // scope.variables をそのまま使用するのではなく、Scope にまとめて後で参照
    // 一旦 temporary_scope を作って参照を保持
    let temporary_scope = Scope {
        identifier_map: BTreeMap::new(), // 未使用
        variable_indices: variable_indices_temp.clone(),
        variable_name_to_var_index: variable_name_to_var_index_temp.clone(),
        variables: scope.variables.clone(), // Clone が必要
        variable_count: slot_index,
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
            &temporary_scope.variable_name_to_var_index,
            &temporary_scope.variables,
            is_function_scope,
        );
        new_resolver
    } else {
        let mut new_resolver = ScopeResolver::new();
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variable_name_to_var_index,
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
            Statement::VariableDeclaration(_, init, _, _) => {
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
#[path = "tests.rs"]
mod tests;
