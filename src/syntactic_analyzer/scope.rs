use std::collections::BTreeMap;

use super::exectree::ExecStatement;

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone)] // Copy
pub struct Identifier {
    scope: usize,
    local: usize,
}

pub struct Variable {
    // NOTE: ここに初期化情報は置かない。型などを置く
}

pub struct Function {
    pub args: Vec<Identifier>,
    pub block: Block,
}

// NOTE: Variable か Function で分けたが、static か const か variable かの分け方の方が
// 関数型実装時に便利。
pub enum Entity {
    Variable(Variable),
    Function(Function),
}

// ----------------------------

pub struct Block {
    pub scope: Scope,
    pub code: Vec<ExecStatement>,
}

#[derive(PartialEq, Eq)]
pub enum ScopeType {
    Global,
    Function,
    Block,
}

pub struct Scope {
    pub scope_identifier: usize, // Scope を識別するもの
    scope_type: ScopeType,
    identifier_map: BTreeMap<Identifier, Entity>,
}

impl Scope {
    pub fn iter_identifier(&self) -> std::collections::btree_map::Iter<'_, Identifier, Entity> {
        self.identifier_map.iter()
    }
    pub fn get(&self, id: &Identifier) -> Option<&Entity> {
        self.identifier_map.get(id)
    }
    pub fn get_variable(&self, id: &Identifier) -> Option<&Variable> {
        if let Some(Entity::Variable(v)) = self.identifier_map.get(id) {
            Some(v)
        } else {
            None
        }
    }
    pub fn get_function(&self, id: &Identifier) -> Option<&Function> {
        if let Some(Entity::Function(v)) = self.identifier_map.get(id) {
            Some(v)
        } else {
            None
        }
    }
}
