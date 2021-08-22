use std::collections::BTreeMap;

use crate::tree_parser::Statement;

struct IdentifierInfo {
    // name: String,
    idx: usize, // TODO: more safety
}

enum Identifier {
    Function(IdentifierInfo),
    Variable(IdentifierInfo),
}

pub struct Variable {
    // NOTE: ここに初期化情報は置きにくい。変数引数としての利用を想定した場合。
// init: Box<Expression>,  // TODO: separate init process
}

pub struct Function {
    pub args: Vec<String>, // TODO: change string to identifier_ptr
    pub scope: Scope,
    pub code: Vec<Statement>, // TODO: convert to analyzed
}

pub struct Scope {
    identifier_map: BTreeMap<String, Identifier>,
    variables: Vec<Variable>,
    functions: Vec<Function>,
}

impl Scope {
    fn new() -> Scope {
        Scope {
            identifier_map: BTreeMap::new(),
            variables: vec![],
            functions: vec![],
        }
    }

    fn add_identifier(&mut self, name: String, identifier: Identifier) {
        if self.identifier_map.contains_key(&name) {
            panic!("syntactic error: the name is already used");
        }
        self.identifier_map.insert(name, identifier);
    }

    // TODO: create ScopeFactory ???
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

fn syntactic_analyze_internal(statements: &Vec<Statement>, is_root: bool) -> Scope {
    let mut scope = Scope::new();
    for stat in statements {
        match stat {
            Statement::VariableDeclaration(name, _init) => {
                if is_root {
                    panic!("todo: global variable is not implemented")
                }
                // TODO: THE VARIABLE INITIALIZER IS LOST HERE
                scope.add_variable(name.clone(), Variable {});
            }
            Statement::FunctionDeclaration(name, args, block) => {
                let mut s = syntactic_analyze_internal(block, false);
                // add variable definition to scope
                for a in args {
                    s.add_variable(a.clone(), Variable {});
                }
                // store variable identifier to function
                let func = Function {
                    args: args.clone(),
                    scope: s,
                    code: block.clone(),
                };
                scope.add_function(name.clone(), func);
            }
            Statement::Return(_) => {
                if is_root {
                    panic!("syntactic error: invalid return in root")
                }
            }
            Statement::Expression(_) => {
                if is_root {
                    panic!("syntactic error: invalid expression in root")
                }
            }
            Statement::Invalid(_) => (),
        }
    }
    scope
}

pub fn syntactic_analyze(root: &Vec<Statement>) -> Scope {
    syntactic_analyze_internal(root, true)
}
