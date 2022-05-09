//! # Syntactic Analyzer
//!
//! このモジュールは、主に定義や宣言の整理をします。
//! このモジュールに処理されると、識別子は全て解決され、定義された変数はいつ確保・解放されるべきか明確になります。
//!

use crate::{
    syntactic_analyzer::pending_exectree::PendingExecExpression,
    tree_parser::{Expression, Statement},
};

pub use self::exectree::{ExecExpression, ExecStatement};
pub use self::scope::{Entity, Function, Identifier, Scope};
use self::{
    pending_exectree::PendingExecStatement,
    pending_scope::{
        PendingBlock, PendingFunction, PendingScope, PendingScopeBuilder, PendingVariable,
    },
    scope::ScopeType,
};

mod exectree;
mod pending_exectree;
mod pending_scope;
mod scope;

fn build_scope_builder(p: (PendingScopeBuilder, Vec<PendingExecStatement>)) -> PendingBlock {
    PendingBlock {
        scope: p.0.build(),
        code: p.1,
    }
}

struct SyntacticAnalyzer {
    scope_identifier_next: usize,
}

impl SyntacticAnalyzer {
    fn parse(root: &Vec<Statement>) -> PendingScope {
        let mut sa = SyntacticAnalyzer {
            scope_identifier_next: 0,
        };
        sa.syntactic_analyze_internal(root, ScopeType::Global)
            .0
            .build()
    }

    fn next_scope_identifier(&mut self) -> usize {
        let id = self.scope_identifier_next;
        self.scope_identifier_next += 1;
        id
    }

    fn convert_to_exec_expression(&mut self, expr: &Box<Expression>) -> Box<PendingExecExpression> {
        match expr.as_ref() {
            Expression::Operation1(op, x) => Box::new(PendingExecExpression::Operation1(
                op.to_owned(),
                self.convert_to_exec_expression(&x),
            )),
            Expression::Operation2(op, l, r) => Box::new(PendingExecExpression::Operation2(
                op.to_owned(),
                self.convert_to_exec_expression(&l),
                self.convert_to_exec_expression(&r),
            )),
            Expression::If(cond, stat1, stat2) => Box::new(PendingExecExpression::If(
                self.convert_to_exec_expression(cond),
                build_scope_builder(self.syntactic_analyze_internal(stat1, ScopeType::Block)),
                build_scope_builder(self.syntactic_analyze_internal(stat2, ScopeType::Block)),
            )),
            Expression::While(expr, stat) => Box::new(PendingExecExpression::While(
                self.convert_to_exec_expression(expr),
                build_scope_builder(self.syntactic_analyze_internal(stat, ScopeType::Block)),
            )),
            Expression::Function(f, a) => Box::new(PendingExecExpression::Function(
                f.to_owned(),
                a.iter()
                    .map(|e| self.convert_to_exec_expression(e))
                    .collect(),
            )),
            Expression::Factor(v) => Box::new(PendingExecExpression::Factor(v.to_owned())),
            Expression::Variable(v) => Box::new(PendingExecExpression::Variable(v.to_owned())),
            Expression::Invalid(_) => todo!(),
        }
    }

    fn syntactic_analyze_internal(
        &mut self,
        statements: &Vec<Statement>,
        scope_type: ScopeType,
    ) -> (PendingScopeBuilder, Vec<PendingExecStatement>) {
        let mut scope = PendingScopeBuilder::new(self.next_scope_identifier(), scope_type);
        let mut exec_statements = Vec::<PendingExecStatement>::new();
        for stat in statements {
            match stat {
                Statement::VariableDeclaration(name, init) => {
                    if let ScopeType::Block = scope_type {
                        panic!("todo: block scoped variable is not implemented")
                    }
                    if let ScopeType::Global = scope_type {
                        panic!("todo: global variable is not implemented")
                    }
                    scope.add_variable(name.clone(), PendingVariable {});
                    exec_statements.push(PendingExecStatement::Expression(
                        self.convert_to_exec_expression(init),
                    ));
                }
                Statement::FunctionDeclaration(name, args, block) => {
                    if let ScopeType::Block = scope_type {
                        panic!("syntactic error: invalid return in block")
                    }
                    let (mut s, es) = self.syntactic_analyze_internal(block, ScopeType::Function);
                    // add variable definition to scope
                    for a in args {
                        s.add_variable(a.clone(), PendingVariable {});
                    }
                    let block = PendingBlock {
                        scope: s.build(),
                        code: es,
                    };
                    // store variable identifier to function
                    let func = PendingFunction {
                        args: args
                            .iter()
                            .map(|a| block.scope.scope_dictionary.get(a).unwrap().to_owned()) // TODO: refactoring
                            .collect(),
                        block: block,
                    };
                    scope.add_function(name.clone(), func);
                }
                Statement::Return(e) => {
                    if let ScopeType::Global = scope_type {
                        panic!("syntactic error: invalid return in root")
                    }
                    exec_statements.push(PendingExecStatement::Return(
                        self.convert_to_exec_expression(e),
                    ));
                }
                Statement::Expression(e) => {
                    if let ScopeType::Global = scope_type {
                        panic!("syntactic error: invalid expression in root")
                    }
                    exec_statements.push(PendingExecStatement::Expression(
                        self.convert_to_exec_expression(e),
                    ));
                }
                Statement::Continue => {
                    if let ScopeType::Global = scope_type {
                        panic!("syntactic error: invalid return in root")
                    }
                    exec_statements.push(PendingExecStatement::Continue);
                }
                Statement::Break => {
                    if let ScopeType::Global = scope_type {
                        panic!("syntactic error: invalid return in root")
                    }
                    exec_statements.push(PendingExecStatement::Break);
                }
                Statement::Invalid(_) => (),
            }
        }
        (scope, exec_statements)
    }
}

pub fn syntactic_analyze(root: &Vec<Statement>) -> Scope {
    let pending_scope = SyntacticAnalyzer::parse(root);
    pending_scope.resolve()
}
