//! # Noop Test Pass
//!
//! フレームワークの動作検証用ダミー最適化パス。
//! グローバル変数 `__opt_marker` を追加し、マジックナンバー `0xDEAD` (57005) で初期化する。
//! 実行結果には影響しない。

use crate::base::SourceLocation;
use crate::semantic_analyzer::{
    ExecExpression, ExecStatement, LocatedExecExpression, LocatedExecStatement, Program, ValueType,
    Variable,
};
use crate::tree_parser::Operator2;

/// マジックナンバー定数
pub const MAGIC_NUMBER: i64 = 0xDEAD;

/// マーカー変数名
pub const MARKER_VAR_NAME: &str = "__opt_marker";

/// ダミー最適化パスを適用する
///
/// ルートスコープにグローバル変数 `__opt_marker` を追加し、
/// `__opt_marker = 0xDEAD` の初期化文を `root_statements` に追加する。
pub fn apply(program: &mut Program) {
    // グローバル領域の末尾（ルートの変数・すべての static 変数の後）に 1 スロット足す
    let slot_index = program.global_slot_count;
    program.global_slot_count += 1;

    // Variable を追加。ルートの変数の後ろではなくグローバル領域の末尾に置くので、
    // 関数・ブロックの static 変数と同じく global_slot で表し、ルートのフレームのスロット（variable_count）は使わない
    // （例外的に、global_slot 付きの変数をルートの variables に入れる）
    let var = Variable {
        slot_index,
        is_static: false,
        is_final: false,
        array_size: None,
        value_type: ValueType::Int,
        global_slot: Some(slot_index),
    };
    let root_scope = &mut program.root_scope;
    let var_index = root_scope.variables.len();
    root_scope.variables.push(var);
    root_scope
        .variable_indices
        .insert(MARKER_VAR_NAME.to_string(), slot_index);
    root_scope
        .variable_name_to_var_index
        .insert(MARKER_VAR_NAME.to_string(), var_index);

    // 初期化文を生成: __opt_marker = 0xDEAD
    // IdentifierRef: scope_depth=0, local_index=slot_index, is_global=true
    let var_ref = crate::semantic_analyzer::IdentifierRef {
        scope_depth: 0,
        local_index: slot_index,
        is_global: true,
    };

    // optimizer が生成する文には位置情報がない（ソースに対応しない）
    let dummy_location = SourceLocation::new(0, 0);

    let init_expr = ExecExpression::Operation2(
        Operator2::Assign,
        Box::new(LocatedExecExpression {
            expression: ExecExpression::Variable(var_ref, ValueType::Int),
            location: dummy_location.clone(),
        }),
        Box::new(LocatedExecExpression {
            expression: ExecExpression::Factor(MAGIC_NUMBER),
            location: dummy_location.clone(),
        }),
    );

    let init_stmt = LocatedExecStatement {
        statement: ExecStatement::Expression(Box::new(LocatedExecExpression {
            expression: init_expr,
            location: dummy_location.clone(),
        })),
        location: dummy_location,
    };

    program.root_statements.push(init_stmt);
}
