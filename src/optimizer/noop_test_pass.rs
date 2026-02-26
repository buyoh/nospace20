//! # Noop Test Pass
//!
//! フレームワークの動作検証用ダミー最適化パス。
//! グローバル変数 `__opt_marker` を追加し、マジックナンバー `0xDEAD` (57005) で初期化する。
//! 実行結果には影響しない。

use crate::base::SourceLocation;
use crate::semantic_analyzer::{ExecExpression, ExecStatement, LocatedExecExpression, LocatedExecStatement, Scope, Variable};
use crate::tree_parser::Operator2;

/// マジックナンバー定数
pub const MAGIC_NUMBER: i64 = 0xDEAD;

/// マーカー変数名
pub const MARKER_VAR_NAME: &str = "__opt_marker";

/// ダミー最適化パスを適用する
///
/// ルートスコープにグローバル変数 `__opt_marker` を追加し、
/// `__opt_marker = 0xDEAD` の初期化文を `root_statements` に追加する。
pub fn apply(scope: &mut Scope) {
    // 新しい変数のスロットインデックス = 現在の variable_count
    let slot_index = scope.variable_count;

    // Variable を追加
    let var = Variable {
        slot_index,
        is_static: false,
        array_size: None,
    };
    let var_index = scope.variables.len();
    scope.variables.push(var);
    scope.variable_count += 1;
    scope.variable_indices.insert(MARKER_VAR_NAME.to_string(), slot_index);
    scope.variable_name_to_var_index.insert(MARKER_VAR_NAME.to_string(), var_index);

    // 初期化文を生成: __opt_marker = 0xDEAD
    // IdentifierRef: scope_depth=0, local_index=slot_index, is_global=true
    let var_ref = crate::semantic_analyzer::IdentifierRef {
        scope_depth: 0,
        local_index: slot_index,
        is_global: true,
        owning_func_index: None,
    };

    // optimizer が生成する文には位置情報がない（ソースに対応しない）
    let dummy_location = SourceLocation::new(0, 0);

    let init_expr = ExecExpression::Operation2(
        Operator2::Assign,
        Box::new(LocatedExecExpression {
            expression: ExecExpression::Variable(var_ref),
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

    scope.root_statements.push(init_stmt);
}
