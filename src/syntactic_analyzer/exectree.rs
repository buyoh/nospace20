use crate::tree_parser::{Operator1, Operator2};

use super::scope::{Block, Identifier};

pub enum ExecExpression {
    Operation1(Operator1, Box<ExecExpression>),
    Operation2(Operator2, Box<ExecExpression>, Box<ExecExpression>),
    If(Box<ExecExpression>, Block, Block),
    While(Box<ExecExpression>, Block),
    Function(Identifier, Vec<Box<ExecExpression>>),
    Factor(i64),
    Variable(Identifier),
}

pub enum ExecStatement {
    Return(Box<ExecExpression>),
    Break,
    Continue,
    Expression(Box<ExecExpression>),
}
