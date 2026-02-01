//! # Semantic Analyzer
//!
//! 意味解析器。ASTを実行可能な構造に変換する。
//!
//! 主な責務:
//! - 変数・関数の識別子解決
//! - スコープ構造の構築
//! - 実行可能な中間表現への変換

mod converter;
mod types;

#[cfg(test)]
mod test;

pub use types::{ExecExpression, ExecStatement, Function, Scope, Variable};
use types::ScopeType;

use crate::tree_parser::Statement;

pub fn analyze(root: &Vec<Statement>) -> Scope {
    converter::analyze_internal(root, ScopeType::Root).0.build()
    // TODO: validate identifiers
}
