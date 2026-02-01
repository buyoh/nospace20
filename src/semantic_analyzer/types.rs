//! # Semantic Analyzer Types
//!
//! 意味解析器の型定義を提供する。

use std::collections::BTreeMap;

use crate::tree_parser::{Operator1, Operator2};

pub(crate) struct IdentifierInfo {
    // name: String,
    pub(crate) idx: usize, // TODO: more safety
}

pub(crate) enum Identifier {
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

pub struct Function {
    pub args: Vec<String>, // TODO: change string to identifier_ptr
    pub scope: Scope,
    pub code: Vec<ExecStatement>,
    // pub identifier: String,
}

pub struct Scope {
    pub(crate) identifier_map: BTreeMap<String, Identifier>,
    pub variables: Vec<Variable>,
    pub(crate) functions: Vec<Function>,
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

pub(crate) enum ScopeType {
    Root,
    Function,
    Block,
}

pub(crate) struct ScopeBuilder {
    pub(crate) identifier_map: BTreeMap<String, Identifier>,
    pub(crate) variables: Vec<Variable>,
    pub(crate) functions: Vec<Function>,
}

impl ScopeBuilder {
    pub(crate) fn new() -> Self {
        Self {
            identifier_map: BTreeMap::new(),
            variables: vec![],
            functions: vec![],
        }
    }

    pub(crate) fn build(self) -> Scope {
        Scope {
            identifier_map: self.identifier_map,
            variables: self.variables,
            functions: self.functions,
        }
    }

    pub(crate) fn add_identifier(&mut self, name: String, identifier: Identifier) {
        if self.identifier_map.contains_key(&name) {
            panic!("semantic error: the name is already used");
        }
        self.identifier_map.insert(name, identifier);
    }

    pub(crate) fn add_variable(&mut self, name: String, var: Variable) {
        let vi = self.variables.len();
        self.variables.push(var);
        self.add_identifier(name, Identifier::Variable(IdentifierInfo { idx: vi }));
    }

    pub(crate) fn add_function(&mut self, name: String, func: Function) {
        let fi = self.functions.len();
        self.functions.push(func);
        self.add_identifier(name, Identifier::Function(IdentifierInfo { idx: fi }));
    }
}
