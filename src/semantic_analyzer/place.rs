//! # 場所（アドレスを持つ式）の判定と代入の左辺の検査
//!
//! 場所は `&` でアドレスを取れる式で、代入の左辺になれる式でもある。
//! 場所の判定は変換後の `ExecExpression` で行う（alias・constexpr・ブロックエイリアスが解決済みで、
//! WS コンパイラが受け取る形と一致するため）。
//! 代入の左辺の検査は「場所であり struct 型でない」（変換後の式）と「final でない」（AST の名前）の 2 つに分ける。

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{Expression, LocatedExpression, Operator1},
};

use super::{
    scope::ScopeResolver,
    types::{ExecExpression, LocatedExecExpression, ValueType},
};

/// 式が場所（アドレスを持ち、`&` でアドレスを取れる式）かどうか
///
/// 場所は変数・配列要素・デリファレンス・構造体のフィールド・構造体の配列フィールドの要素の 5 種類。
/// 型注釈 `(arr)@P` は値（アドレス）であって場所ではない（そのフィールド `(arr)@P.f` は場所）。
pub(super) fn is_place(expr: &ExecExpression) -> bool {
    matches!(
        expr,
        ExecExpression::Variable(_, _)
            | ExecExpression::ArrayAccess(_, _, _)
            | ExecExpression::Operation1(Operator1::Deref, _)
            | ExecExpression::StructFieldAccess(_, _, _, _)
            | ExecExpression::StructFieldArrayAccess(_, _, _, _)
    )
}

/// 変換済みの代入の左辺が代入できる式か検査する（場所であり、struct 型でない）
///
/// final の検査は `check_not_final_target` で別に行う。
pub(super) fn check_assignable_target(
    lhs: &LocatedExecExpression,
    func_return_types: &[ValueType],
    location_start: usize,
) -> Result<(), Vec<CodeParseError>> {
    if !is_place(&lhs.expression) {
        return Err(vec![code_parse_error!(
            location_start,
            "left-hand side of assignment must be a variable, array element, struct field or dereference"
        )]);
    }
    // 構造体は代入不可（spec）。配列型の場所（arr・s.g）は先頭要素への代入として許す
    if matches!(lhs.infer_type(func_return_types), ValueType::Struct(_)) {
        return Err(vec![code_parse_error!(
            location_start,
            "cannot assign to struct value"
        )]);
    }
    Ok(())
}

/// 代入の左辺（AST）が final 変数・final 変数の配列要素でないか検査する
///
/// 名前を直接書いた代入だけを検査する。`*ptr = value` のような間接代入・構造体のフィールドは検査しない。
pub(super) fn check_not_final_target(
    lhs: &LocatedExpression,
    resolver: &ScopeResolver,
    location_start: usize,
) -> Result<(), Vec<CodeParseError>> {
    let (name, message_prefix) = match &lhs.expression {
        Expression::Variable(name) => (name, "cannot assign to final variable"),
        Expression::ArrayAccess(name, _) => (name, "cannot assign to element of final array"),
        _ => return Ok(()),
    };
    let resolved_name = resolver
        .resolve_alias_chain(name)
        .map_err(|e| vec![code_parse_error!(location_start, e)])?;
    if resolver.is_final_variable(&resolved_name) {
        return Err(vec![code_parse_error!(
            location_start,
            format!("{} '{}'", message_prefix, name)
        )]);
    }
    Ok(())
}
