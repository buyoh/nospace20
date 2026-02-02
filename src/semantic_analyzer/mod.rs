//! # Semantic Analyzer
//!
//! 意味解析器。ASTを実行可能な構造に変換する。
//!
//! 主な責務:
//! - 変数・関数の識別子解決
//! - スコープ構造の構築
//! - 実行可能な中間表現への変換

use std::collections::BTreeMap;

use crate::tree_parser::{Expression, Operator1, Operator2, Statement};

struct IdentifierInfo {
    // name: String,
    idx: usize, // TODO: more safety
}

enum Identifier {
    Function(IdentifierInfo),
    Variable(IdentifierInfo),
}

pub struct Variable {
    // NOTE: ここに初期化情報は置かない
    pub identifier: String, // TODO: use IdentifierInfo
}

/// 実行可能な式を表す。
///
/// `Expression` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない (パース成功後のみ生成される)
/// - 将来的にはスコープ解決済みの識別子情報を保持する予定
///   (例: 変数名の文字列ではなく、スコープ内のインデックスを保持)
///
/// 現状は `Expression` と構造が類似しているが、意味解析の責務拡張に伴い差異が生じる想定。
// #[derive(Clone)] // TODO: REMOVE
pub enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Vec<ExecStatement>, Vec<ExecStatement>),
    While(Box<ExecExpression>, Vec<ExecStatement>),
    Function(String, Vec<Box<ExecExpression>>),
    Factor(i64),
    Variable(String), // TODO: スコープ解決済みの IdentifierInfo に変更予定
}

/// 実行可能な文を表す。
///
/// `Statement` (構文解析結果) との違い:
/// - `Invalid` バリアントを持たない
/// - 宣言文 (VariableDeclaration, FunctionDeclaration) を持たない
///   (宣言は `Scope` 構造に変換される)
// #[derive(Clone)] // TODO: REMOVE
pub enum ExecStatement {
    Return(Box<ExecExpression>),
    Break,
    Continue,
    Expression(Box<ExecExpression>),
}

fn convert_to_exec_expression(expr: &Box<Expression>) -> Box<ExecExpression> {
    match expr.as_ref() {
        Expression::Operation1(op, x) => Box::new(ExecExpression::Operation1(
            op.to_owned(),
            convert_to_exec_expression(&x),
        )),
        Expression::Operation2(op, l, r) => Box::new(ExecExpression::Operation2(
            op.to_owned(),
            convert_to_exec_expression(&l),
            convert_to_exec_expression(&r),
        )),
        Expression::If(cond, stat1, stat2) => Box::new(ExecExpression::If(
            convert_to_exec_expression(cond),
            analyze_internal(stat1, ScopeType::Block).1,
            analyze_internal(stat2, ScopeType::Block).1,
        )),
        Expression::While(expr, stat) => Box::new(ExecExpression::While(
            convert_to_exec_expression(expr),
            analyze_internal(stat, ScopeType::Block).1,
        )),
        Expression::Function(f, a) => Box::new(ExecExpression::Function(
            f.to_owned(),
            a.iter().map(|e| convert_to_exec_expression(e)).collect(),
        )),
        Expression::Factor(v) => Box::new(ExecExpression::Factor(v.to_owned())),
        Expression::Variable(v) => Box::new(ExecExpression::Variable(v.to_owned())),
        // パースエラー時のみ Invalid が生成されるため、正常系では到達しない
        Expression::Invalid(_) => {
            unreachable!("Expression::Invalid should not reach semantic analysis")
        }
    }
}

pub struct Function {
    pub args: Vec<String>, // TODO: change string to identifier_ptr
    pub scope: Scope,
    pub code: Vec<ExecStatement>,
    // pub identifier: String,
}

pub struct Scope {
    identifier_map: BTreeMap<String, Identifier>,
    pub variables: Vec<Variable>,
    functions: Vec<Function>,
}

impl Scope {
    pub fn get_function(&self, id: &str) -> Option<&Function> {
        if let Some(Identifier::Function(info)) = self.identifier_map.get(id) {
            Some(&self.functions[info.idx])
        } else {
            None
        }
    }

    pub fn get_variable(&self, id: &str) -> Option<&Variable> {
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

    fn build(self) -> Scope {
        Scope {
            identifier_map: self.identifier_map,
            variables: self.variables,
            functions: self.functions,
        }
    }

    fn add_identifier(&mut self, name: String, identifier: Identifier) {
        if self.identifier_map.contains_key(&name) {
            panic!("semantic error: the name is already used");
        }
        self.identifier_map.insert(name, identifier);
    }

    fn add_variable(&mut self, name: String, var: Variable) {
        let vi = self.variables.len();
        self.variables.push(var);
        self.add_identifier(name, Identifier::Variable(IdentifierInfo { idx: vi }));
    }

    fn add_function(&mut self, name: String, func: Function) {
        let fi = self.functions.len();
        self.functions.push(func);
        self.add_identifier(name, Identifier::Function(IdentifierInfo { idx: fi }));
    }
}

fn analyze_internal(
    statements: &Vec<Statement>,
    scope_type: ScopeType,
) -> (ScopeBuilder, Vec<ExecStatement>) {
    let mut scope = ScopeBuilder::new();
    let mut exec_statements = Vec::<ExecStatement>::new();
    for stat in statements {
        match stat {
            Statement::VariableDeclaration(name, init) => {
                if let ScopeType::Block = scope_type {
                    // TODO(unimplemented): ブロックスコープ変数は未実装
                    panic!("todo: block scoped variable is not implemented")
                }
                if let ScopeType::Root = scope_type {
                    // TODO(unimplemented): グローバル変数は未実装
                    panic!("todo: global variable is not implemented")
                }
                scope.add_variable(
                    name.clone(),
                    Variable {
                        identifier: name.clone(),
                    },
                );
                exec_statements.push(ExecStatement::Expression(convert_to_exec_expression(init)));
            }
            Statement::FunctionDeclaration(name, args, block) => {
                if let ScopeType::Block = scope_type {
                    // TODO(error-handling): Result型でエラーを返すべき (ネスト関数宣言は未対応)
                    panic!("semantic error: nested function declaration is not supported")
                }
                let (mut s, es) = analyze_internal(block, ScopeType::Function);
                // add variable definition to scope
                for a in args {
                    s.add_variable(
                        a.clone(),
                        Variable {
                            identifier: a.clone(),
                        },
                    );
                }
                // store variable identifier to function
                let func = Function {
                    args: args.clone(),
                    scope: s.build(),
                    code: es,
                };
                scope.add_function(name.clone(), func);
            }
            Statement::Return(e) => {
                if let ScopeType::Root = scope_type {
                    // TODO(error-handling): Result型でエラーを返すべき
                    panic!("semantic error: return statement outside of function")
                }
                exec_statements.push(ExecStatement::Return(convert_to_exec_expression(e)));
            }
            Statement::Expression(e) => {
                if let ScopeType::Root = scope_type {
                    // TODO(error-handling): Result型でエラーを返すべき
                    panic!("semantic error: expression statement at root level")
                }
                exec_statements.push(ExecStatement::Expression(convert_to_exec_expression(e)));
            }
            Statement::Continue => {
                if let ScopeType::Root = scope_type {
                    // TODO(error-handling): Result型でエラーを返すべき
                    panic!("semantic error: continue statement outside of function")
                }
                exec_statements.push(ExecStatement::Continue);
            }
            Statement::Break => {
                if let ScopeType::Root = scope_type {
                    // TODO(error-handling): Result型でエラーを返すべき
                    panic!("semantic error: break statement outside of function")
                }
                exec_statements.push(ExecStatement::Break);
            }
            Statement::Invalid(_) => (),
        }
    }
    (scope, exec_statements)
}

pub fn analyze(root: &Vec<Statement>) -> Scope {
    analyze_internal(root, ScopeType::Root).0.build()
    // TODO: validate identifiers
}
