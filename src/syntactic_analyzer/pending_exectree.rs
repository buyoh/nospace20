use crate::tree_parser::{Operator1, Operator2};

use super::{pending_scope::PendingBlock, scope::Identifier};

pub enum PendingIdentifier {
    Resolved(Identifier),
    // Note: 一旦 NotResolvedとしておき、2週目で解決する
    NotResolved(String),
}

// #[derive(Clone)] // TODO: REMOVE
pub enum PendingExecExpression {
    Operation1(Operator1, Box<PendingExecExpression>),
    Operation2(
        Operator2,
        Box<PendingExecExpression>,
        Box<PendingExecExpression>,
    ),
    If(Box<PendingExecExpression>, PendingBlock, PendingBlock),
    While(Box<PendingExecExpression>, PendingBlock),
    Function(String, Vec<Box<PendingExecExpression>>),
    Factor(i64),
    Variable(String),
}

// #[derive(Clone)] // TODO: REMOVE
pub enum PendingExecStatement {
    Return(Box<PendingExecExpression>),
    Break,
    Continue,
    Expression(Box<PendingExecExpression>),
}
