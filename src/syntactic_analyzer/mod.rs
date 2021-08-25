use std::collections::BTreeMap;

use crate::tree_parser::{Expression, Statement};

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

#[derive(Clone)] // TODO: REMOVE
pub enum ExecStatement {
    Return(Box<Expression>),
    Expression(Box<Expression>),
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
    pub fn get_function(&self, id: &String) -> Option<&Function> {
        if let Some(Identifier::Function(info)) = self.identifier_map.get(id) {
            Some(&self.functions[info.idx])
        } else {
            None
        }
    }

    pub fn get_variable(&self, id: &String) -> Option<&Variable> {
        if let Some(Identifier::Variable(info)) = self.identifier_map.get(id) {
            Some(&self.variables[info.idx])
        } else {
            None
        }
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

    fn build(self) -> Scope {
        Scope {
            identifier_map: self.identifier_map,
            variables: self.variables,
            functions: self.functions,
        }
    }

    fn add_identifier(&mut self, name: String, identifier: Identifier) {
        if self.identifier_map.contains_key(&name) {
            panic!("syntactic error: the name is already used");
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

fn syntactic_analyze_internal(
    statements: &Vec<Statement>,
    is_root: bool,
) -> (ScopeBuilder, Vec<ExecStatement>) {
    let mut scope = ScopeBuilder::new();
    let mut exec_statements = Vec::<ExecStatement>::new();
    for stat in statements {
        match stat {
            Statement::VariableDeclaration(name, init) => {
                if is_root {
                    panic!("todo: global variable is not implemented")
                }
                scope.add_variable(
                    name.clone(),
                    Variable {
                        identifier: name.clone(),
                    },
                );
                exec_statements.push(ExecStatement::Expression(init.clone()));
            }
            Statement::FunctionDeclaration(name, args, block) => {
                let (mut s, es) = syntactic_analyze_internal(block, false);
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
                if is_root {
                    panic!("syntactic error: invalid return in root")
                }
                exec_statements.push(ExecStatement::Return(e.clone()));
            }
            Statement::Expression(e) => {
                if is_root {
                    panic!("syntactic error: invalid expression in root")
                }
                exec_statements.push(ExecStatement::Expression(e.clone()));
            }
            Statement::Invalid(_) => (),
        }
    }
    (scope, exec_statements)
}

pub fn syntactic_analyze(root: &Vec<Statement>) -> Scope {
    syntactic_analyze_internal(root, true).0.build()
    // TODO: validate identifiers
}
