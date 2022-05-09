use std::collections::BTreeMap;

use super::{
    exectree::{ExecExpression, ExecStatement},
    pending_exectree::{PendingExecExpression, PendingExecStatement},
    scope::{Block, Entity, Function, Identifier, ScopeType, Variable},
    Scope,
};

// ----------------------------

pub struct PendingVariable {}

pub struct PendingFunction {
    pub args: Vec<Identifier>,
    pub block: PendingBlock,
}

pub enum PendingEntity {
    Variable(PendingVariable),
    Function(PendingFunction),
}

// ----------------------------

pub struct PendingBlock {
    pub scope: PendingScope,
    pub code: Vec<PendingExecStatement>,
}

// ----------------------------

pub struct PendingScopeBuilder {
    identifier_next: usize,
    scope_identifier: usize,
    scope_type: ScopeType,
    dictionary_map: BTreeMap<String, Identifier>,
    identifier_map: BTreeMap<Identifier, PendingEntity>,
}

impl PendingScopeBuilder {
    pub fn new(scope_identifier: usize, scope_type: ScopeType) -> Self {
        Self {
            identifier_next: 0,
            scope_identifier,
            scope_type,
            dictionary_map: BTreeMap::new(),
            identifier_map: BTreeMap::new(),
        }
    }

    pub fn build(self) -> PendingScope {
        PendingScope {
            scope_identifier: self.scope_identifier,
            scope_type: self.scope_type,
            identifier_map: self.identifier_map,
            scope_dictionary: self.dictionary_map,
        }
    }

    fn create_identifier(&mut self, name: String) -> Identifier {
        if self.dictionary_map.contains_key(&name) {
            panic!("syntactic error: the name is already used");
        }
        let id = self.identifier_next;
        self.identifier_next += 1;
        let identifier = Identifier {
            scope: self.scope_identifier,
            local: id,
        };
        self.dictionary_map.insert(name, identifier.to_owned());
        identifier
    }

    pub fn add_variable(&mut self, name: String, var: PendingVariable) {
        self.identifier_map
            .insert(self.create_identifier(name), PendingEntity::Variable(var));
    }

    pub fn add_function(&mut self, name: String, func: PendingFunction) {
        self.identifier_map
            .insert(self.create_identifier(name), PendingEntity::Function(func));
    }
}

// ----------------------------

pub struct PendingScope {
    scope_identifier: usize,
    scope_type: ScopeType,
    identifier_map: BTreeMap<Identifier, PendingEntity>,
    pub scope_dictionary: BTreeMap<String, Identifier>, // String 状態のidentifierの解決に使う
}

impl PendingScope {
    pub fn resolve(mut self) -> Scope {
        let resolver = ScopeStackResolver::new_with_root(&self);
        self.resolve_internal(&resolver)
    }

    fn resolve_internal(&self, resolver: &ScopeStackResolver) -> Scope {
        // self.scope_dictionary.dictionary_map
        Scope {
            scope_identifier: self.scope_identifier,
            scope_type: self.scope_type,
            identifier_map: self
                .identifier_map
                .into_iter()
                .map(|(key, id)| (key, resolve_entity(resolver, &id)))
                .collect(),
        }
    }
}

// ----------------------------

struct ScopeStackResolver<'a>(Vec<&'a PendingScope>);
// - String から Identifier と Entity を得る
// - 違反する Identifier の解決は Err を返す。以下が違反となる。
//  -

impl<'a> ScopeStackResolver<'a> {
    fn new() -> Self {
        Self(vec![])
    }

    fn new_with_root(scope: &'a PendingScope) -> Self {
        Self(vec![scope])
    }

    fn pushed(&self, scope: &'a PendingScope) -> Self {
        let v2 = self.0.to_owned();
        v2.push(scope);
        Self(v2)
    }

    fn resolve(&self, id_str: &String) -> Result<(Identifier, &PendingEntity), &str> {
        let mut out_of_func = false; // foldで出来なくもないが…
        if let Some((identifier, &scope, out_of_func)) = self.0.iter().rev().find_map(|s| {
            s.scope_dictionary
                .get(id_str)
                .and_then(|id| Some((id, s, out_of_func)))
                .or_else(|| {
                    out_of_func |= s.scope_type != ScopeType::Block;
                    None
                })
        }) {
            let entity = scope.identifier_map.get(identifier).unwrap();
            match entity {
                PendingEntity::Function(f) => Ok((identifier.to_owned(), entity)),
                PendingEntity::Variable(v) => {
                    if out_of_func {
                        Err("syntactic error: cannot access variables over function scope")
                    } else {
                        Ok((identifier.to_owned(), entity))
                    }
                }
            }
        } else {
            Err(format!("syntactic error: unknown identifier: {}", id_str).as_str())
        }
    }

    fn resolve_function(&self, id_str: &String) -> Result<(Identifier, &PendingFunction), &str> {
        self.resolve(id_str).and_then(|(id, entity)| {
            if let PendingEntity::Function(f) = entity {
                Ok((id, f))
            } else {
                Err("syntactic error: the identifier is not function")
            }
        })
    }

    fn resolve_variable(&self, id_str: &String) -> Result<(Identifier, &PendingVariable), &str> {
        self.resolve(id_str).and_then(|(id, entity)| {
            if let PendingEntity::Variable(v) = entity {
                Ok((id, v))
            } else {
                Err("syntactic error: the identifier is not variable")
            }
        })
    }
}

// ----------------------------

fn resolve_entity<'a>(resolver: &'a ScopeStackResolver<'a>, id: &PendingEntity) -> Entity {
    match id {
        PendingEntity::Function(f) => Entity::Function(Function {
            args: f.args,
            block: resolve_block(resolver, &f.block),
        }),
        PendingEntity::Variable(v) => Entity::Variable(Variable {}),
    }
}

fn resolve_block<'a>(resolver: &'a ScopeStackResolver<'a>, block: &'a PendingBlock) -> Block {
    let deep_resolver = resolver.pushed(&block.scope);
    let code = block
        .code
        .iter()
        .map(|statement| resolve_statement(&deep_resolver, statement))
        .collect();
    Block {
        scope: block.scope.resolve_internal(&resolver),
        code,
    }
}

fn resolve_statement<'a>(
    resolver: &'a ScopeStackResolver<'a>,
    statement: &'a PendingExecStatement,
) -> ExecStatement {
    match statement {
        PendingExecStatement::Return(expression) => {
            ExecStatement::Return(resolve_expression(resolver, expression))
        }
        PendingExecStatement::Break => ExecStatement::Break,
        PendingExecStatement::Continue => ExecStatement::Continue,
        PendingExecStatement::Expression(expression) => {
            ExecStatement::Expression(resolve_expression(resolver, expression))
        }
    }
}

fn resolve_expression<'a>(
    resolver: &'a ScopeStackResolver<'a>,
    expression: &'a PendingExecExpression,
) -> Box<ExecExpression> {
    match expression {
        PendingExecExpression::Operation1(op, e) => Box::new(ExecExpression::Operation1(
            *op,
            resolve_expression(resolver, e),
        )),
        PendingExecExpression::Operation2(op, e1, e2) => Box::new(ExecExpression::Operation2(
            *op,
            resolve_expression(resolver, e1),
            resolve_expression(resolver, e2),
        )),
        PendingExecExpression::If(e, b1, b2) => Box::new(ExecExpression::If(
            resolve_expression(resolver, e),
            resolve_block(resolver, b1),
            resolve_block(resolver, b2),
        )),
        PendingExecExpression::While(e, b1) => Box::new(ExecExpression::While(
            resolve_expression(resolver, e),
            resolve_block(resolver, b1),
        )),
        PendingExecExpression::Function(id_str, args) => Box::new(ExecExpression::Function(
            resolver.resolve_function(id_str).unwrap().0, // TODO: remove unwrap
            args.iter()
                .map(|e| resolve_expression(resolver, e))
                .collect(),
        )),
        PendingExecExpression::Factor(v) => Box::new(ExecExpression::Factor(*v)),
        PendingExecExpression::Variable(id_str) => {
            Box::new(ExecExpression::Variable(
                resolver.resolve_variable(id_str).unwrap().0,
            )) // TODO: remove unwrap
        }
    }
}
