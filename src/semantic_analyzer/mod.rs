//! # Semantic Analyzer
//!
//! 意味解析器。ASTを実行可能な構造に変換する。
//!
//! 主な責務:
//! - 変数・関数の識別子解決
//! - スコープ構造の構築
//! - 実行可能な中間表現への変換

mod scope;
mod types;

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use scope::{FunctionIndex, Identifier, ScopeBuilder, ScopeResolver, ScopeType, SymbolTable};

use crate::{
    base::{constexpr_eval, pure_eval, CodeParseError, SourceLocation},
    code_parse_error,
    tree_parser::{
        AliasArg, AliasParamKind, Expression, LocatedExpression, LocatedStatement, Operator1,
        Operator2, Statement,
    },
};

pub use scope::{Function, Scope};
pub(crate) use types::{
    Block, ConditionMode, ExecExpression, ExecStatement, InternalBuiltinFunctionKind,
    LocatedExecExpression, LocatedExecStatement, Variable,
};
pub use types::{BuiltinFunctionKind, IdentifierRef, ValueType};

/// constexpr 式を再帰的に評価する
///
/// `raw` は未解決の constexpr 定義（名前 → 生式）。
/// `resolved` は解決済みの constexpr 定数テーブル（名前 → 値）。
/// `evaluating` は巡回参照検知用の「現在評価中」セット。
fn evaluate_constexpr_expr(
    expr: &LocatedExpression,
    raw: &BTreeMap<String, Box<LocatedExpression>>,
    resolved: &mut BTreeMap<String, i64>,
    evaluating: &mut BTreeSet<String>,
) -> Result<i64, Vec<CodeParseError>> {
    let loc = expr.location.start;
    match &expr.expression {
        Expression::Factor(n) => Ok(*n),
        Expression::Variable(name) => {
            if let Some(&v) = resolved.get(name) {
                return Ok(v);
            }
            if raw.contains_key(name) {
                // 前方参照: 他の constexpr を評価する
                return evaluate_constexpr_by_name(name, raw, resolved, evaluating);
            }
            Err(vec![code_parse_error!(
                loc,
                format!("'{}' is not a compile-time constant", name)
            )])
        }
        Expression::Operation1(op, inner) => match op {
            Operator1::Negative => {
                let v = evaluate_constexpr_expr(inner, raw, resolved, evaluating)?;
                Ok(v.wrapping_neg())
            }
            Operator1::LogicalNot => {
                let v = evaluate_constexpr_expr(inner, raw, resolved, evaluating)?;
                Ok(pure_eval::bool_to_int(v == 0))
            }
            _ => Err(vec![code_parse_error!(
                loc,
                "Ref/Deref is not allowed in constexpr expression"
            )]),
        },
        Expression::Operation2(op, l, r) => match op {
            Operator2::Assign
            | Operator2::PlusAssign
            | Operator2::MinusAssign
            | Operator2::MultiplyAssign
            | Operator2::DivideAssign
            | Operator2::ModuloAssign => Err(vec![code_parse_error!(
                loc,
                "assignment is not allowed in constexpr expression"
            )]),
            Operator2::LogicalAnd => {
                // 短絡評価: 左辺が0なら右辺を評価しない
                let lv = evaluate_constexpr_expr(l, raw, resolved, evaluating)?;
                if lv == 0 {
                    return Ok(0);
                }
                let rv = evaluate_constexpr_expr(r, raw, resolved, evaluating)?;
                Ok(pure_eval::bool_to_int(rv != 0))
            }
            Operator2::LogicalOr => {
                // 短絡評価: 左辺が非0なら右辺を評価しない
                let lv = evaluate_constexpr_expr(l, raw, resolved, evaluating)?;
                if lv != 0 {
                    return Ok(1);
                }
                let rv = evaluate_constexpr_expr(r, raw, resolved, evaluating)?;
                Ok(pure_eval::bool_to_int(rv != 0))
            }
            _ => {
                // Plus, Minus, Multiply, Divide, Modulo, 比較演算
                let lv = evaluate_constexpr_expr(l, raw, resolved, evaluating)?;
                let rv = evaluate_constexpr_expr(r, raw, resolved, evaluating)?;
                pure_eval::eval_binary_pure(op, lv, rv).ok_or_else(|| {
                    vec![code_parse_error!(
                        loc,
                        "division by zero in constexpr expression"
                    )]
                })
            }
        },
        _ => Err(vec![code_parse_error!(
            loc,
            "expression is not compile-time evaluable in constexpr"
        )]),
    }
}

/// constexpr 名前による遅延解決〔巡回参照検知付き〕
fn evaluate_constexpr_by_name(
    name: &str,
    raw: &BTreeMap<String, Box<LocatedExpression>>,
    resolved: &mut BTreeMap<String, i64>,
    evaluating: &mut BTreeSet<String>,
) -> Result<i64, Vec<CodeParseError>> {
    if let Some(&v) = resolved.get(name) {
        return Ok(v);
    }
    if evaluating.contains(name) {
        return Err(vec![code_parse_error!(format!(
            "circular constexpr reference detected: '{}' is part of a cyclic definition",
            name
        ))]);
    }
    let expr = match raw.get(name) {
        Some(e) => e.clone(),
        None => {
            return Err(vec![code_parse_error!(format!(
                "undefined constexpr: '{}'",
                name
            ))])
        }
    };
    evaluating.insert(name.to_string());
    let v = match &expr.expression {
        Expression::Block(stmts) => {
            // ブロック形式: base/constexpr_eval を使用して評価
            // resolved テーブルを ConstexprEnv に渡し、解決済み定数を参照可能にする
            let mut env = constexpr_eval::ConstexprEnv::new(resolved);
            constexpr_eval::eval_constexpr_block(stmts, &mut env)?
        }
        _ => {
            // 式形式: 既存のロジックを使用
            evaluate_constexpr_expr(&expr, raw, resolved, evaluating)?
        }
    };
    evaluating.remove(name);
    resolved.insert(name.to_string(), v);
    Ok(v)
}

/// ステートメント列から constexpr 定義を収集して評価し、
/// 定数テーブル `BTreeMap<String, i64>` を返す。
///
/// 同名の constexpr や変数との名前衝突は意味解析パス1b での重複チェックに委ねる。
fn collect_constexpr_table(
    statements: &[LocatedStatement],
) -> Result<BTreeMap<String, i64>, Vec<CodeParseError>> {
    // 生式マップ（名前 → 生式）を構築
    let mut raw: BTreeMap<String, Box<LocatedExpression>> = BTreeMap::new();
    for located_stat in statements {
        if let Statement::ConstexprDeclaration(name, expr) = &located_stat.statement {
            raw.insert(name.clone(), expr.clone());
        }
    }

    if raw.is_empty() {
        return Ok(BTreeMap::new());
    }

    // 各 constexpr を遅延評価
    let mut resolved: BTreeMap<String, i64> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    for name in raw.keys() {
        let mut evaluating: BTreeSet<String> = BTreeSet::new();
        match evaluate_constexpr_by_name(name, &raw, &mut resolved, &mut evaluating) {
            Ok(_) => {}
            Err(mut errs) => errors.append(&mut errs),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(resolved)
}

/// ステートメント列から alias（識別子エイリアス）定義を収集し、
/// エイリアステーブル `BTreeMap<String, String>` を返す。
///
/// 重複定義はエラーとして報告する。
fn collect_alias_map(
    statements: &[LocatedStatement],
) -> Result<BTreeMap<String, String>, Vec<CodeParseError>> {
    let mut alias_map: BTreeMap<String, String> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    for located_stat in statements {
        if let Statement::AliasIdentifier(name, target) = &located_stat.statement {
            if alias_map.contains_key(name) {
                errors.push(code_parse_error!(
                    located_stat.location.start,
                    format!("duplicate alias definition: '{}'", name)
                ));
            } else {
                alias_map.insert(name.clone(), target.clone());
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(alias_map)
}

/// ステートメント列からブロックエイリアス定義を収集し、
/// ブロックエイリアステーブル `BTreeMap<String, Vec<LocatedStatement>>` を返す。
///
/// 重複定義・識別子エイリアスとの名前衝突はエラーとして報告する。
fn collect_block_alias_map(
    statements: &[LocatedStatement],
    alias_map: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Vec<LocatedStatement>>, Vec<CodeParseError>> {
    let mut block_alias_map: BTreeMap<String, Vec<LocatedStatement>> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    for located_stat in statements {
        if let Statement::AliasBlock(name, body) = &located_stat.statement {
            if block_alias_map.contains_key(name) {
                errors.push(code_parse_error!(
                    located_stat.location.start,
                    format!("duplicate block alias definition: '{}'", name)
                ));
            } else if alias_map.contains_key(name) {
                errors.push(code_parse_error!(
                    located_stat.location.start,
                    format!(
                        "alias '{}' is defined as both identifier alias and block alias",
                        name
                    )
                ));
            } else {
                block_alias_map.insert(name.clone(), body.clone());
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(block_alias_map)
}

/// ブロックエイリアスの AST を走査し、直接参照する他のブロックエイリアス名のセットを返す
fn collect_block_alias_refs_in_stmts(
    stmts: &[LocatedStatement],
    block_alias_map: &BTreeMap<String, Vec<LocatedStatement>>,
    alias_map: &BTreeMap<String, String>,
    refs: &mut BTreeSet<String>,
) {
    for stat in stmts {
        collect_block_alias_refs_in_stmt(stat, block_alias_map, alias_map, refs);
    }
}

fn collect_block_alias_refs_in_stmt(
    stat: &LocatedStatement,
    block_alias_map: &BTreeMap<String, Vec<LocatedStatement>>,
    alias_map: &BTreeMap<String, String>,
    refs: &mut BTreeSet<String>,
) {
    match &stat.statement {
        Statement::Expression(expr) => {
            collect_block_alias_refs_in_expr(expr, block_alias_map, alias_map, refs)
        }
        Statement::Return(Some(expr)) => {
            collect_block_alias_refs_in_expr(expr, block_alias_map, alias_map, refs)
        }
        Statement::While(cond, body) => {
            collect_block_alias_refs_in_expr(cond, block_alias_map, alias_map, refs);
            collect_block_alias_refs_in_stmts(body, block_alias_map, alias_map, refs);
        }
        Statement::For(init, cond, step, body) => {
            collect_block_alias_refs_in_stmts(init, block_alias_map, alias_map, refs);
            collect_block_alias_refs_in_stmts(cond, block_alias_map, alias_map, refs);
            collect_block_alias_refs_in_stmts(step, block_alias_map, alias_map, refs);
            collect_block_alias_refs_in_stmts(body, block_alias_map, alias_map, refs);
        }
        Statement::VariableDeclaration(_, expr, _, _, _) => {
            collect_block_alias_refs_in_expr(expr, block_alias_map, alias_map, refs)
        }
        Statement::AliasBlock(_, body) => {
            // ネストしたブロックエイリアス定義内も走査しない（別スコープ）
            let _ = body;
        }
        _ => {}
    }
}

fn collect_block_alias_refs_in_expr(
    expr: &LocatedExpression,
    block_alias_map: &BTreeMap<String, Vec<LocatedStatement>>,
    alias_map: &BTreeMap<String, String>,
    refs: &mut BTreeSet<String>,
) {
    match &expr.expression {
        Expression::Function(name, args) => {
            // alias チェーン解決した上でブロックエイリアスかどうかを確認
            let mut resolved = name.clone();
            let mut visited = BTreeSet::new();
            loop {
                if visited.contains(&resolved) {
                    break;
                }
                visited.insert(resolved.clone());
                if let Some(target) = alias_map.get(&resolved) {
                    resolved = target.clone();
                } else {
                    break;
                }
            }
            if block_alias_map.contains_key(&resolved) {
                refs.insert(resolved);
            }
            for arg in args {
                collect_block_alias_refs_in_expr(arg, block_alias_map, alias_map, refs);
            }
        }
        Expression::Operation1(_, inner) => {
            collect_block_alias_refs_in_expr(inner, block_alias_map, alias_map, refs)
        }
        Expression::Operation2(_, l, r) => {
            collect_block_alias_refs_in_expr(l, block_alias_map, alias_map, refs);
            collect_block_alias_refs_in_expr(r, block_alias_map, alias_map, refs);
        }
        Expression::If(cond, then_stmts, else_stmts) => {
            collect_block_alias_refs_in_expr(cond, block_alias_map, alias_map, refs);
            collect_block_alias_refs_in_stmts(then_stmts, block_alias_map, alias_map, refs);
            collect_block_alias_refs_in_stmts(else_stmts, block_alias_map, alias_map, refs);
        }
        Expression::Block(stmts) => {
            collect_block_alias_refs_in_stmts(stmts, block_alias_map, alias_map, refs)
        }
        _ => {}
    }
}

/// ブロックエイリアスの巡回参照を DFS で検知する
///
/// 同一スコープ内のブロックエイリアス定義間の依存グラフを走査し、
/// 巡回参照がある場合はコンパイルエラーを返す。
fn detect_block_alias_cycles(
    block_alias_map: &BTreeMap<String, Vec<LocatedStatement>>,
    alias_map: &BTreeMap<String, String>,
) -> Result<(), Vec<CodeParseError>> {
    // 依存グラフを構築: 各ブロックエイリアスが参照する他のブロックエイリアスのセット
    let mut dep_graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, body) in block_alias_map {
        let mut refs = BTreeSet::new();
        collect_block_alias_refs_in_stmts(body, block_alias_map, alias_map, &mut refs);
        dep_graph.insert(name.clone(), refs);
    }

    // DFS で巡回を検知
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut errors: Vec<CodeParseError> = Vec::new();

    fn dfs(
        node: &str,
        dep_graph: &BTreeMap<String, BTreeSet<String>>,
        visited: &mut BTreeSet<String>,
        path: &mut Vec<String>,
        errors: &mut Vec<CodeParseError>,
    ) {
        if let Some(pos) = path.iter().position(|x| x == node) {
            // 巡回検知: path[pos..] が巡回しているサイクル
            let cycle: Vec<&str> = path[pos..].iter().map(|s| s.as_str()).collect();
            let chain = cycle.join(" → ");
            errors.push(code_parse_error!(format!(
                "recursive block alias expansion detected: {} → {}",
                chain, node
            )));
            return;
        }
        if visited.contains(node) {
            return;
        }
        path.push(node.to_string());
        if let Some(deps) = dep_graph.get(node) {
            for dep in deps {
                dfs(dep, dep_graph, visited, path, errors);
            }
        }
        path.pop();
        visited.insert(node.to_string());
    }

    for name in block_alias_map.keys() {
        let mut path = Vec::new();
        dfs(name, &dep_graph, &mut visited, &mut path, &mut errors);
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(())
}

/// 関数本体に return: 文が存在するか再帰的にチェックする
///
/// ネストした if/while/block の中もすべてチェックするが、ネストされた関数宣言の中は除外する
fn has_return_statement(statements: &[LocatedStatement]) -> bool {
    for stat in statements {
        match &stat.statement {
            Statement::Return(Some(_)) => return true,
            Statement::Return(None) => {} // void return は int 返却とみなさない
            Statement::Expression(expr) => {
                if expr_contains_return(&expr.expression) {
                    return true;
                }
            }
            Statement::While(_, stmts) => {
                if has_return_statement(stmts) {
                    return true;
                }
            }
            Statement::For(init, cond, step, body) => {
                for block in &[init, cond, step, body] {
                    if has_return_statement(block) {
                        return true;
                    }
                }
            }
            // ネストした関数宣言は除外（別の関数の return なので）
            Statement::FunctionDeclaration(_, _, _) => {}
            _ => {}
        }
    }
    false
}

/// 式の中に return: 文が含まれるかチェックする。if/block 内の return: を再帰的にチェック
fn expr_contains_return(expr: &crate::tree_parser::Expression) -> bool {
    use crate::tree_parser::Expression;
    match expr {
        Expression::If(_, then_stmts, else_stmts) => {
            has_return_statement(then_stmts) || has_return_statement(else_stmts)
        }
        Expression::Block(stmts) => has_return_statement(stmts),
        _ => false,
    }
}

/// 関数本体がすべての制御パスで return を保証するかチェックする
///
/// 軽量な到達可能性チェック（完全な制御フロー解析ではない）:
/// - 最後の文が Return → true
/// - 最後の文が if-else（else あり）かつ両ブランチが保証 → true
/// - それ以外 → false
fn guarantees_return(statements: &[LocatedStatement]) -> bool {
    match statements.last() {
        None => false,
        Some(last) => match &last.statement {
            Statement::Return(Some(_)) => true,
            Statement::Return(None) => false, // void return は値の返却を保証しない
            Statement::Expression(expr) => expr_guarantees_return(&expr.expression),
            _ => false,
        },
    }
}

/// 式がすべての制御パスで return を保証するかチェックする
fn expr_guarantees_return(expr: &crate::tree_parser::Expression) -> bool {
    use crate::tree_parser::Expression;
    match expr {
        Expression::If(_, then_stmts, else_stmts) => {
            // else なし（空の else_stmts）の if は保証しない
            if else_stmts.is_empty() {
                return false;
            }
            // 両方のブランチが保証する場合のみ保証
            guarantees_return(then_stmts) && guarantees_return(else_stmts)
        }
        Expression::Block(stmts) => guarantees_return(stmts),
        _ => false,
    }
}

/// void 型の式が値として使われている場合にエラーを返す
fn require_int_type(
    expr: &LocatedExecExpression,
    func_return_types: &[ValueType],
) -> Result<(), Vec<CodeParseError>> {
    if expr.infer_type(func_return_types) == ValueType::Void {
        Err(vec![code_parse_error!(
            "semantic error: cannot use void expression as a value"
        )])
    } else {
        Ok(())
    }
}

/// LocatedExecExpression を構築するヘルパー
fn make_located_exec(
    expr: ExecExpression,
    location: &SourceLocation,
) -> Box<LocatedExecExpression> {
    Box::new(LocatedExecExpression {
        expression: expr,
        location: location.clone(),
    })
}

/// 式を ExecExpression に変換する（識別子解決あり）
///
/// ScopeResolver を使用して変数名・関数名を IdentifierRef に解決する。
/// func_return_types を使用して式の型チェックを行う。
fn convert_to_exec_expression_with_resolver(
    located_expr: &Box<LocatedExpression>,
    parent_resolver: &ScopeResolver,
    func_return_types: &[ValueType],
) -> Result<Box<LocatedExecExpression>, Vec<CodeParseError>> {
    let loc = &located_expr.location;
    let expr = &located_expr.expression;
    match expr {
        Expression::Operation1(Operator1::Ref, inner) => {
            // & は変数または配列要素に対してのみ使用可能
            match &inner.expression {
                Expression::Variable(name) => {
                    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                        vec![code_parse_error!(
                            loc.start,
                            format!("undefined variable: {}", name)
                        )]
                    })?;
                    Ok(make_located_exec(
                        ExecExpression::Operation1(
                            Operator1::Ref,
                            make_located_exec(ExecExpression::Variable(id_ref), &inner.location),
                        ),
                        loc,
                    ))
                }
                Expression::ArrayAccess(name, index_expr) => {
                    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                        vec![code_parse_error!(
                            loc.start,
                            format!("undefined variable: {}", name)
                        )]
                    })?;

                    // arr[i] は *(&arr + i) と同義。配列でなくてもインデックスアクセス可能。
                    let array_size = parent_resolver
                        .get_array_size(name)
                        .ok_or_else(|| {
                            vec![code_parse_error!(
                                loc.start,
                                format!("undefined variable: {}", name)
                            )]
                        })?
                        .unwrap_or(1);

                    let exec_index = convert_to_exec_expression_with_resolver(
                        index_expr,
                        parent_resolver,
                        func_return_types,
                    )?;

                    Ok(make_located_exec(
                        ExecExpression::Operation1(
                            Operator1::Ref,
                            make_located_exec(
                                ExecExpression::ArrayAccess(id_ref, exec_index, array_size),
                                &inner.location,
                            ),
                        ),
                        loc,
                    ))
                }
                _ => Err(vec![code_parse_error!(
                    loc.start,
                    "reference operator (&) can only be applied to variables or array elements"
                )]),
            }
        }
        Expression::Operation1(op, x) => {
            let exec_x =
                convert_to_exec_expression_with_resolver(&x, parent_resolver, func_return_types)?;
            // void 型の式は単項演算のオペランドに使用不可
            require_int_type(&exec_x, func_return_types)?;
            Ok(make_located_exec(
                ExecExpression::Operation1(op.to_owned(), exec_x),
                loc,
            ))
        }
        Expression::Operation2(op, l, r) => {
            // 複合代入演算子 (+=, -=, *=, /=, %=) を a = a + b の形式に展開
            let (actual_op, actual_l, actual_r) = match op {
                Operator2::PlusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(Operator2::Plus, l.clone(), r.clone()),
                        location: loc.clone(),
                    }),
                ),
                Operator2::MinusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(Operator2::Minus, l.clone(), r.clone()),
                        location: loc.clone(),
                    }),
                ),
                Operator2::MultiplyAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(
                            Operator2::Multiply,
                            l.clone(),
                            r.clone(),
                        ),
                        location: loc.clone(),
                    }),
                ),
                Operator2::DivideAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(Operator2::Divide, l.clone(), r.clone()),
                        location: loc.clone(),
                    }),
                ),
                Operator2::ModuloAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(Operator2::Modulo, l.clone(), r.clone()),
                        location: loc.clone(),
                    }),
                ),
                _ => (op.to_owned(), l, r),
            };

            // final 変数への代入チェック: 再代入不可の変数への書き込みはコンパイルエラー
            if actual_op == Operator2::Assign {
                match &actual_l.expression {
                    Expression::Variable(name) => {
                        let resolved_name = parent_resolver.resolve_alias_chain(name).map_err(|e| {
                            vec![code_parse_error!(loc.start, e)]
                        })?;
                        if parent_resolver.is_final_variable(&resolved_name) {
                            return Err(vec![code_parse_error!(
                                loc.start,
                                format!("cannot assign to final variable '{}'", name)
                            )]);
                        }
                    }
                    Expression::ArrayAccess(name, _) => {
                        let resolved_name = parent_resolver.resolve_alias_chain(name).map_err(|e| {
                            vec![code_parse_error!(loc.start, e)]
                        })?;
                        if parent_resolver.is_final_variable(&resolved_name) {
                            return Err(vec![code_parse_error!(
                                loc.start,
                                format!("cannot assign to element of final array '{}'", name)
                            )]);
                        }
                    }
                    _ => {
                        // *ptr = value のような間接代入は静的チェック不可
                    }
                }
            }

            let exec_l = convert_to_exec_expression_with_resolver(
                &actual_l,
                parent_resolver,
                func_return_types,
            )?;
            let exec_r = convert_to_exec_expression_with_resolver(
                &actual_r,
                parent_resolver,
                func_return_types,
            )?;

            // 型チェック: void 式が二項演算のオペランドに使用されている場合はエラー
            match actual_op {
                Operator2::Assign => {
                    // 代入の右辺は Int でなければならない
                    require_int_type(&exec_r, func_return_types)?;
                }
                _ => {
                    // その他の演算: 両辺は Int でなければならない
                    require_int_type(&exec_l, func_return_types)?;
                    require_int_type(&exec_r, func_return_types)?;
                }
            }

            Ok(make_located_exec(
                ExecExpression::Operation2(actual_op, exec_l, exec_r),
                loc,
            ))
        }
        Expression::If(cond, stat1, stat2) => {
            let exec_cond =
                convert_to_exec_expression_with_resolver(cond, parent_resolver, func_return_types)?;
            // void 型の式は条件式に使用不可
            require_int_type(&exec_cond, func_return_types)?;
            let (s1, es1) = analyze_internal_with_parent(
                stat1,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
                // Phase 5: If/While の中ではglobal_functionsを使わない（関数宣言不可）
                // だだし、型の一貫性のために仮の可変参照を渡す
                &mut Vec::new(),
                &mut Vec::new(),
                None,
                func_return_types.to_vec(),
            )?;
            let (s2, es2) = analyze_internal_with_parent(
                stat2,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
                &mut Vec::new(),
                &mut Vec::new(),
                None,
                func_return_types.to_vec(),
            )?;
            Ok(make_located_exec(
                ExecExpression::If(
                    ConditionMode::NonZero,
                    exec_cond,
                    Block {
                        scope: s1.build(Vec::new(), Vec::new(), Vec::new()), // root_statementsは空
                        statements: es1,
                    },
                    Block {
                        scope: s2.build(Vec::new(), Vec::new(), Vec::new()), // root_statementsは空
                        statements: es2,
                    },
                ),
                loc,
            ))
        }
        Expression::Block(statements) => {
            let (s, es) = analyze_internal_with_parent(
                statements,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
                &mut Vec::new(),
                &mut Vec::new(),
                None,
                func_return_types.to_vec(),
            )?;
            Ok(make_located_exec(
                ExecExpression::Block(Block {
                    scope: s.build(Vec::new(), Vec::new(), Vec::new()), // root_statementsは空
                    statements: es,
                }),
                loc,
            ))
        }
        Expression::Function(f, a) => {
            // Phase 5: 組み込み関数とユーザー定義関数を区別
            let mut args = Vec::new();
            for e in a {
                let exec_arg = convert_to_exec_expression_with_resolver(
                    e,
                    parent_resolver,
                    func_return_types,
                )?;
                // 引数は void 型不可
                require_int_type(&exec_arg, func_return_types)?;
                args.push(exec_arg);
            }

            // 組み込み関数のリスト（__ で始まる）
            // Phase 6: 文字列を BuiltinFunctionKind に変換
            let builtin_kind = match f.as_str() {
                "__puti" => Some(types::BuiltinFunctionKind::Puti),
                "__putc" => Some(types::BuiltinFunctionKind::Putc),
                "__geti" => Some(types::BuiltinFunctionKind::Geti),
                "__getc" => Some(types::BuiltinFunctionKind::Getc),
                "__clog" => Some(types::BuiltinFunctionKind::Clog),
                "__assert" => Some(types::BuiltinFunctionKind::Assert),
                "__assert_not" => Some(types::BuiltinFunctionKind::AssertNot),
                "__trace" => Some(types::BuiltinFunctionKind::Trace),
                "__alloc" => Some(types::BuiltinFunctionKind::Alloc),
                "__free" => Some(types::BuiltinFunctionKind::Free),
                _ => None,
            };

            if let Some(kind) = builtin_kind {
                // 組み込み関数の引数数チェック
                let expected = match kind {
                    types::BuiltinFunctionKind::Puti => 1,
                    types::BuiltinFunctionKind::Putc => 1,
                    types::BuiltinFunctionKind::Geti => 0,
                    types::BuiltinFunctionKind::Getc => 0,
                    types::BuiltinFunctionKind::Clog => 1,
                    types::BuiltinFunctionKind::Assert => 1,
                    types::BuiltinFunctionKind::AssertNot => 1,
                    types::BuiltinFunctionKind::Trace => 1,
                    types::BuiltinFunctionKind::Alloc => 1,
                    types::BuiltinFunctionKind::Free => 1,
                };
                if args.len() != expected {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        format!(
                            "builtin function '{}' expects {} argument(s), but {} were provided",
                            f,
                            expected,
                            args.len()
                        )
                    )]);
                }
                // 組み込み関数
                Ok(make_located_exec(
                    ExecExpression::BuiltinFunction(kind, args),
                    loc,
                ))
            } else {
                // ユーザー定義関数：まず alias を解決してから resolve する
                let resolved_f = parent_resolver.resolve_alias_chain(f).map_err(|e| {
                    vec![code_parse_error!(loc.start, e)]
                })?;

                // ブロックエイリアスのチェック: alias チェーン解決後の名前で検索
                if let Some(block_body) = parent_resolver.resolve_block_alias(&resolved_f) {
                    // ブロックエイリアスに引数は不可
                    if !args.is_empty() {
                        return Err(vec![code_parse_error!(
                            loc.start,
                            format!(
                                "block alias '{}' cannot be called with arguments",
                                f
                            )
                        )]);
                    }
                    // ブロックエイリアスをインライン展開: 呼び出し元スコープで本体を解析
                    let block_body_clone = block_body.clone();
                    let (s, es) = analyze_internal_with_parent(
                        &block_body_clone,
                        ScopeType::Block,
                        Vec::new(),
                        Some(parent_resolver),
                        &mut Vec::new(),
                        &mut Vec::new(),
                        None,
                        func_return_types.to_vec(),
                    )?;
                    return Ok(make_located_exec(
                        ExecExpression::Block(Block {
                            scope: s.build(Vec::new(), Vec::new(), Vec::new()),
                            statements: es,
                        }),
                        loc,
                    ));
                }

                let func_ref = parent_resolver.resolve_function(&resolved_f).ok_or_else(|| {
                    vec![code_parse_error!(
                        loc.start,
                        format!("undefined function: {}", f)
                    )]
                })?;

                // 引数数チェック
                let expected_count = parent_resolver
                    .get_function_arg_count(&resolved_f)
                    .expect("function should be resolvable");
                if args.len() != expected_count {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        format!(
                            "function '{}' expects {} argument(s), but {} were provided",
                            f,
                            expected_count,
                            args.len()
                        )
                    )]);
                }

                Ok(make_located_exec(
                    ExecExpression::UserFunction(func_ref, args),
                    loc,
                ))
            }
        }
        Expression::Factor(v) => Ok(make_located_exec(ExecExpression::Factor(v.to_owned()), loc)),
        Expression::Variable(v) => {
            // まず alias を解決（チェーン解決）
            let resolved_name = parent_resolver.resolve_alias_chain(v).map_err(|e| {
                vec![code_parse_error!(loc.start, e)]
            })?;

            // まず constexpr テーブルを確認（定数式への置換）
            if let Some(const_val) = parent_resolver.resolve_constexpr(&resolved_name) {
                return Ok(make_located_exec(ExecExpression::Factor(const_val), loc));
            }
            // 変数名を解決
            let var_ref = parent_resolver.resolve_variable(&resolved_name).ok_or_else(|| {
                vec![code_parse_error!(
                    loc.start,
                    format!("undefined variable: {}", v)
                )]
            })?;
            Ok(make_located_exec(ExecExpression::Variable(var_ref), loc))
        }
        Expression::ArrayAccess(name, index_expr) => {
            let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                vec![code_parse_error!(
                    loc.start,
                    format!("undefined variable: {}", name)
                )]
            })?;

            // arr[i] は *(&arr + i) と同義。配列でなくてもインデックスアクセス可能。
            let array_size = parent_resolver
                .get_array_size(name)
                .ok_or_else(|| {
                    vec![code_parse_error!(
                        loc.start,
                        format!("undefined variable: {}", name)
                    )]
                })?
                .unwrap_or(1);

            let exec_index = convert_to_exec_expression_with_resolver(
                index_expr,
                parent_resolver,
                func_return_types,
            )?;
            // 配列インデックスに void 型は使用不可
            require_int_type(&exec_index, func_return_types)?;

            Ok(make_located_exec(
                ExecExpression::ArrayAccess(id_ref, exec_index, array_size),
                loc,
            ))
        }
        // パースエラー時のみ Invalid が生成されるため、正常系では到達しない
        Expression::Invalid(_) => {
            unreachable!("Expression::Invalid should not reach semantic analysis")
        }
    }
}

/// テンプレートエントリ（`TemplateFunctionDefinition` から収集）
struct TemplateEntry {
    args: Vec<String>,
    alias_params: Vec<crate::tree_parser::AliasParam>,
    body: Vec<LocatedStatement>,
}

/// テンプレート関数のインスタンス化を展開するプレパス
///
/// ステートメントリストを走査し、以下を行う:
/// 1. `TemplateFunctionDefinition` をテンプレートテーブルに収集
/// 2. `AliasInstantiation` を対応する `FunctionDeclaration` へ展開
/// 3. `AliasIdentifier` のターゲットがテンプレート関数の場合、alias パラメータ数を検証
///
/// 展開後のリストには `TemplateFunctionDefinition` と `AliasInstantiation` は含まれない。
fn expand_template_instantiations(
    statements: &[LocatedStatement],
) -> Result<Vec<LocatedStatement>, Vec<CodeParseError>> {
    // テンプレート定義が存在するか確認（最適化: 存在しない場合は早期リターン）
    let has_templates = statements.iter().any(|s| {
        matches!(s.statement, Statement::TemplateFunctionDefinition { .. })
    });
    let has_instantiations = statements.iter().any(|s| {
        matches!(s.statement, Statement::AliasInstantiation { .. })
    });

    if !has_templates && !has_instantiations {
        return Ok(statements.to_vec());
    }

    // Pass 1: テンプレート定義を収集
    let mut template_map: BTreeMap<String, TemplateEntry> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    for stat in statements {
        if let Statement::TemplateFunctionDefinition { name, args, alias_params, body } = &stat.statement {
            if template_map.contains_key(name.as_str()) {
                errors.push(code_parse_error!(
                    stat.location.start,
                    format!("duplicate template function definition: '{}'", name)
                ));
            } else {
                template_map.insert(name.clone(), TemplateEntry {
                    args: args.clone(),
                    alias_params: alias_params.clone(),
                    body: body.clone(),
                });
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // Pass 2: ステートメントリストを変換
    let mut result: Vec<LocatedStatement> = Vec::with_capacity(statements.len());
    for stat in statements {
        match &stat.statement {
            Statement::TemplateFunctionDefinition { .. } => {
                // テンプレート定義はコード生成対象外 → スキップ
            }
            Statement::AliasInstantiation { name, template_name, alias_args } => {
                // テンプレートを検索
                let template = match template_map.get(template_name.as_str()) {
                    Some(t) => t,
                    None => {
                        // template_name が通常関数の場合もあり得るが、
                        // 引数が2つ以上なので通常の alias は不可 → エラー
                        errors.push(code_parse_error!(
                            stat.location.start,
                            format!("'{}' is not a template function", template_name)
                        ));
                        continue;
                    }
                };

                // alias 引数数の検証
                if alias_args.len() != template.alias_params.len() {
                    errors.push(code_parse_error!(
                        stat.location.start,
                        format!(
                            "alias argument count mismatch for template '{}': expected {}, got {}",
                            template_name,
                            template.alias_params.len(),
                            alias_args.len()
                        )
                    ));
                    continue;
                }

                // インスタンス化: テンプレートボディの先頭に alias/constexpr 文を挿入
                let mut synthetic_body: Vec<LocatedStatement> = Vec::new();
                let loc = stat.location.clone();
                let mut has_error = false;

                for (param, arg) in template.alias_params.iter().zip(alias_args.iter()) {
                    match &param.kind {
                        AliasParamKind::Func(_) => {
                            // alias: func: param_name → `alias: param_name(concrete_func);`
                            match arg {
                                AliasArg::Identifier(func_name) => {
                                    synthetic_body.push(LocatedStatement {
                                        statement: Statement::AliasIdentifier(
                                            param.name.clone(),
                                            func_name.clone(),
                                        ),
                                        location: loc.clone(),
                                    });
                                }
                                AliasArg::Value(_) => {
                                    errors.push(code_parse_error!(
                                        stat.location.start,
                                        format!(
                                            "template '{}': func alias parameter '{}' requires a function name, not an integer literal",
                                            template_name, param.name
                                        )
                                    ));
                                    has_error = true;
                                }
                            }
                        }
                        AliasParamKind::Constexpr => {
                            // alias: constexpr: param_name → `constexpr: param_name(value);`
                            let expr = match arg {
                                AliasArg::Value(n) => Box::new(LocatedExpression {
                                    expression: Expression::Factor(*n),
                                    location: loc.clone(),
                                }),
                                AliasArg::Identifier(cexpr_name) => Box::new(LocatedExpression {
                                    expression: Expression::Variable(cexpr_name.clone()),
                                    location: loc.clone(),
                                }),
                            };
                            synthetic_body.push(LocatedStatement {
                                statement: Statement::ConstexprDeclaration(param.name.clone(), expr),
                                location: loc.clone(),
                            });
                        }
                        AliasParamKind::Static => {
                            // alias: static: param_name → `alias: param_name(static_var_name);`
                            // 実行時に static 変数として機能するかの検証は semantic_analyzer Pass 2 に委譲
                            match arg {
                                AliasArg::Identifier(static_name) => {
                                    synthetic_body.push(LocatedStatement {
                                        statement: Statement::AliasIdentifier(
                                            param.name.clone(),
                                            static_name.clone(),
                                        ),
                                        location: loc.clone(),
                                    });
                                }
                                AliasArg::Value(_) => {
                                    errors.push(code_parse_error!(
                                        stat.location.start,
                                        format!(
                                            "template '{}': static alias parameter '{}' requires a static variable name, not an integer literal",
                                            template_name, param.name
                                        )
                                    ));
                                    has_error = true;
                                }
                            }
                        }
                    }
                }

                if has_error {
                    continue;
                }

                // テンプレートボディを追記
                synthetic_body.extend(template.body.clone());

                // FunctionDeclaration として登録
                result.push(LocatedStatement {
                    statement: Statement::FunctionDeclaration(
                        name.clone(),
                        template.args.clone(),
                        synthetic_body,
                    ),
                    location: stat.location.clone(),
                });
            }
            Statement::AliasIdentifier(name, target) => {
                // ターゲットがテンプレート関数の場合、alias パラメータ数を検証
                if let Some(template) = template_map.get(target.as_str()) {
                    if !template.alias_params.is_empty() {
                        errors.push(code_parse_error!(
                            stat.location.start,
                            format!(
                                "template '{}' requires {} alias argument(s), but 0 were provided; use 'alias: {}({}, ...)' to instantiate",
                                target,
                                template.alias_params.len(),
                                name,
                                target
                            )
                        ));
                        continue;
                    }
                }
                result.push(stat.clone());
            }
            _ => {
                result.push(stat.clone());
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(result)
}

fn analyze_internal(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    global_functions: &mut Vec<Function>,
    global_function_names: &mut Vec<String>,
) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>> {
    analyze_internal_with_parent(
        statements,
        scope_type,
        Vec::new(),
        None,
        global_functions,
        global_function_names,
        None,
        Vec::new(), // inherited_func_return_types: 空 = global_functions から収集
    )
}

/// 初期変数と親のresolve rを指定して解析する
/// Phase 5: global_functions と global_function_names を追加（全関数をルートスコープにフラット化）
fn analyze_internal_with_parent(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    initial_vars: Vec<String>,
    parent_resolver: Option<&ScopeResolver>,
    global_functions: &mut Vec<Function>,
    global_function_names: &mut Vec<String>,
    func_global_index: Option<usize>,
    inherited_func_return_types: Vec<ValueType>,
) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>> {
    // テンプレート関数のインスタンス化を展開するプレパス
    // TemplateFunctionDefinition と AliasInstantiation を FunctionDeclaration に変換する
    let expanded_statements = expand_template_instantiations(statements)?;
    let statements: &Vec<LocatedStatement> = &expanded_statements;

    let mut scope = ScopeBuilder::new();

    // グローバル変数は暗黙的に static
    let is_static = matches!(scope_type, ScopeType::Root);
    let is_function_scope = matches!(scope_type, ScopeType::Root | ScopeType::Function);

    // 初期変数を登録（関数の引数など）
    for var_name in initial_vars {
        scope.add_variable(
            &var_name,
            Variable {
                slot_index: 0,    // build() で正しい値に設定される
                is_static: false, // 関数引数は非 static
                array_size: None, // 関数引数は配列ではない
                is_final: false,  // 関数引数は final 不可
            },
        )?;
    }

    // 3パス解析 → 4パス解析（Pass 0 を追加）
    // パス0: constexpr 定義の収集・評価
    let constexpr_table_temp = collect_constexpr_table(statements)?;
    // パス0: alias（識別子エイリアス）定義の収集
    let alias_map_temp = collect_alias_map(statements)?;
    // パス0: ブロックエイリアス定義の収集
    let block_alias_map_temp = collect_block_alias_map(statements, &alias_map_temp)?;
    // パス0: ブロックエイリアスの巡回参照チェック
    detect_block_alias_cycles(&block_alias_map_temp, &alias_map_temp)?;

    // パス1a: 関数宣言を先にスキャンして登録（ホイスティング対応）
    // Phase 5: ネスト関数のサポート
    // Phase 5 修正: scope.functions ではなく global_functions に登録
    for located_stat in statements {
        let stat = &located_stat.statement;
        match stat {
            Statement::FunctionDeclaration(name, args, body) => {
                // 関数を仮登録（本体は後で解析）
                // とりあえず空の関数をプレースホルダーとして登録
                let global_idx = global_functions.len();

                // 戻り値型を推論: return: 文が存在するか確認
                let has_ret = has_return_statement(body);
                // 混在チェック: return 文があるがすべてのパスで return を保証しない場合はエラー
                // （一部のパスで return あり、別のパスで暗黙の return あり）
                // guarantees_return を使って軽量な制御フロー解析を行う
                if has_ret && !guarantees_return(body) {
                    return Err(vec![code_parse_error!(format!(
                        "semantic error: function '{}' has mixed return types (return in some paths but not all)",
                        name
                    ))]);
                }
                let return_type = if has_ret {
                    ValueType::Int
                } else {
                    ValueType::Void
                };

                global_function_names.push(name.clone());
                global_functions.push(Function {
                    arg_indices: Vec::new(),
                    return_type,
                    is_unused: false,
                    block: Block {
                        scope: Scope {
                            identifier_map: BTreeMap::new(),
                            variable_indices: BTreeMap::new(),
                            variable_name_to_var_index: BTreeMap::new(),
                            variables: Vec::new(),
                            variable_count: 0,
                            functions: Vec::new(),
                            symbol_table: SymbolTable {
                                function_names: Vec::new(),
                                function_name_to_index: BTreeMap::new(),
                            },
                            main_function_index: None,
                            static_init_statements: Vec::new(),
                            root_statements: Vec::new(),
                        },
                        statements: Vec::new(),
                    },
                });
                // identifier_map にはグローバルインデックスと引数数と戻り値型を登録
                scope.add_identifier(
                    name,
                    Identifier::Function(FunctionIndex(global_idx, args.len(), return_type)),
                )?;
            }
            _ => {}
        }
    }

    // 型チェック用の関数戻り値型スライスを決定
    // inherited_func_return_types が空 = ルートまたは関数スコープ → global_functions から収集
    // inherited_func_return_types が非空 = if/while/block の内部 → 外側の型コンテキストを継承
    let effective_func_return_types: Vec<ValueType> = if inherited_func_return_types.is_empty() {
        global_functions.iter().map(|f| f.return_type).collect()
    } else {
        inherited_func_return_types
    };

    // パス1b: 変数宣言収集（ホイスティング対応）
    for located_stat in statements {
        let stat = &located_stat.statement;
        match stat {
            Statement::VariableDeclaration(name, _, is_static_explicit, is_final, array_size) => {
                // グローバル変数は暗黙的に static、明示的 static も考慮
                let final_is_static = *is_static_explicit || is_static;
                scope.add_variable(
                    name,
                    Variable {
                        slot_index: 0, // build() で正しい値に設定される
                        is_static: final_is_static,
                        array_size: array_size.map(|n| n as usize),
                        is_final: *is_final,
                    },
                )?;
            }
            Statement::FunctionDeclaration(_name, _, _) => {
                // パス1aで処理済み
            }
            Statement::ConstexprDeclaration(_, _) => {
                // コンパイル時定数は変数スロットを確保しない - パス0 で処理済み
            }
            Statement::AliasIdentifier(_, _) => {
                // エイリアスはシンボルテーブルに登録しない - パス0 で処理済み
            }
            Statement::AliasBlock(_, _) => {
                // ブロックエイリアスはシンボルテーブルに登録しない - パス0 で処理済み
            }
            _ => {}
        }
    }

    // 変数名からインデックスへのマッピングを先に構築（resolver で使用）
    // 配列サイズを考慮したスロットインデックスを使用
    let mut variable_indices_temp = BTreeMap::new();
    let mut variable_name_to_var_index_temp = BTreeMap::new();
    let mut slot_index = 0;
    for (idx, var) in scope.variables.iter().enumerate() {
        let var_name = &scope.variable_names[idx];
        variable_indices_temp.insert(var_name.clone(), slot_index);
        variable_name_to_var_index_temp.insert(var_name.clone(), idx);
        slot_index += var.array_size.unwrap_or(1);
    }

    // Variable を Clone するための一時保存（resolver が参照するため）
    // scope.variables をそのまま使用するのではなく、Scope にまとめて後で参照
    // 一旦 temporary_scope を作って参照を保持
    // Phase 5: identifier_map も保持して関数解決に使用
    let temporary_scope = Scope {
        identifier_map: scope.identifier_map.clone(), // Phase 5: 関数解決に必要
        variable_indices: variable_indices_temp.clone(),
        variable_name_to_var_index: variable_name_to_var_index_temp.clone(),
        variables: scope.variables.clone(), // Clone が必要
        variable_count: slot_index,
        functions: Vec::new(), // 未使用
        symbol_table: SymbolTable {
            function_names: Vec::new(),
            function_name_to_index: BTreeMap::new(),
        },
        main_function_index: None, // Phase 6: 一時スコープなので None
        static_init_statements: Vec::new(), // 未使用
        root_statements: Vec::new(), // 未使用
    };

    // 親のresolverを継承して新しいresolverを作成
    // Phase 5: func_map を追加
    let mut resolver = if let Some(parent) = parent_resolver {
        let mut new_resolver = ScopeResolver {
            scope_stack: parent.scope_stack.clone(),
        };
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variable_name_to_var_index,
            &temporary_scope.variables,
            &temporary_scope.identifier_map,
            &constexpr_table_temp,
            &alias_map_temp,
            &block_alias_map_temp,
            is_function_scope,
            func_global_index,
        );
        new_resolver
    } else {
        let mut new_resolver = ScopeResolver::new();
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variable_name_to_var_index,
            &temporary_scope.variables,
            &temporary_scope.identifier_map,
            &constexpr_table_temp,
            &alias_map_temp,
            &block_alias_map_temp,
            is_function_scope,
            func_global_index,
        );
        new_resolver
    };

    // パス2: 文の変換（識別子解決を伴う）
    let mut exec_statements = Vec::<LocatedExecStatement>::new();
    for located_stat in statements {
        let stat = &located_stat.statement;
        let loc = &located_stat.location;
        match stat {
            Statement::VariableDeclaration(_name, init, is_static_explicit, _, _) => {
                // 初期化式を変換（変数宣言自体はパス1で完了）
                // final 変数の初期化代入は再代入ブロックの対象外にするため、
                // init_expr のトップレベルの Assign を分解して直接構築する
                let exec_init =
                    if let Expression::Operation2(Operator2::Assign, lhs_expr, rhs_expr) =
                        &init.expression
                    {
                        // 初期化代入: rhs のみ変換し、Assign を直接構築（final チェックなし）
                        let exec_rhs = convert_to_exec_expression_with_resolver(
                            rhs_expr,
                            &resolver,
                            &effective_func_return_types,
                        )?;
                        require_int_type(&exec_rhs, &effective_func_return_types)?;
                        let exec_lhs = convert_to_exec_expression_with_resolver(
                            lhs_expr,
                            &resolver,
                            &effective_func_return_types,
                        )?;
                        make_located_exec(
                            ExecExpression::Operation2(
                                Operator2::Assign,
                                exec_lhs,
                                exec_rhs,
                            ),
                            &init.location,
                        )
                    } else {
                        // 初期値なし（Factor(0)）の場合は通常変換
                        convert_to_exec_expression_with_resolver(
                            init,
                            &resolver,
                            &effective_func_return_types,
                        )?
                    };
                let exec_stmt = ExecStatement::Expression(exec_init);
                let located = LocatedExecStatement {
                    statement: exec_stmt,
                    location: loc.clone(),
                };
                // static 変数の初期化式は分離する
                // - ルートスコープ: static 変数の初期化は非 static より先に実行
                // - 関数スコープ: static 変数の初期化は main 前に1回だけ実行
                if *is_static_explicit {
                    scope.static_init_statements.push(located);
                } else {
                    exec_statements.push(located);
                }
            }
            Statement::FunctionDeclaration(name, args, block) => {
                // Phase 5: パス1aで登録済みの関数のグローバルインデックスを取得
                let global_idx =
                    if let Some(Identifier::Function(info)) = scope.identifier_map.get(name) {
                        info.0
                    } else {
                        panic!("internal error: function should be pre-registered in pass 1a");
                    };

                // 関数本体を解析（親resolverを渡してグローバル変数を参照可能にする）
                // Phase 5: global_functions と global_function_names を渡す
                // func_global_index を渡すことで、ネストされた関数から
                // この関数の static 変数にアクセスする際に正しいオフセットを参照可能にする
                let (s, es) = analyze_internal_with_parent(
                    block,
                    ScopeType::Function,
                    args.clone(),
                    Some(&resolver),
                    global_functions,
                    global_function_names,
                    Some(global_idx),
                    Vec::new(), // 隢数本体の pass1a 完了後に global_functions から収集
                )?;
                // Phase 5: 非ルートスコープの build() には空の functions/function_names を渡す
                let built_scope = s.build(Vec::new(), Vec::new(), Vec::new()); // root_statementsは空

                // 引数のインデックスを事前計算（最適化）
                let arg_indices: Vec<usize> = args
                    .iter()
                    .map(|arg_name| {
                        *built_scope
                            .variable_indices
                            .get(arg_name)
                            .expect("argument must be registered as variable")
                    })
                    .collect();

                // 隢数の戻り値型はパス1aで決定済みの値を使用
                let func_return_type =
                    if let Some(Identifier::Function(info)) = scope.identifier_map.get(name) {
                        info.2
                    } else {
                        panic!("internal error: function return_type should be in pass 1a info");
                    };

                global_functions[global_idx] = Function {
                    arg_indices,
                    return_type: func_return_type,
                    is_unused: false,
                    block: Block {
                        scope: built_scope,
                        statements: es,
                    },
                };
            }
            Statement::Return(e) => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: return statement outside of function"
                    )]);
                }
                match e {
                    Some(expr) => {
                        let exec_e = convert_to_exec_expression_with_resolver(
                            expr,
                            &resolver,
                            &effective_func_return_types,
                        )?;
                        // return: の式は Int でなければならない
                        require_int_type(&exec_e, &effective_func_return_types)?;
                        exec_statements.push(LocatedExecStatement {
                            statement: ExecStatement::Return(Some(exec_e)),
                            location: loc.clone(),
                        });
                    }
                    None => {
                        // void return: 関数が int 型（return: expr; がある）場合はエラー
                        if let Some(idx) = func_global_index {
                            if global_functions[idx].return_type != ValueType::Void {
                                return Err(vec![code_parse_error!(
                                    loc.start,
                                    "semantic error: return without value in non-void function"
                                )]);
                            }
                        }
                        exec_statements.push(LocatedExecStatement {
                            statement: ExecStatement::Return(None),
                            location: loc.clone(),
                        });
                    }
                }
            }
            Statement::Expression(e) => {
                // ルートスコープでも式文を許可（グローバル変数の初期化式）
                // 式文は void 型でも OK（値は捨てられる）
                exec_statements.push(LocatedExecStatement {
                    statement: ExecStatement::Expression(convert_to_exec_expression_with_resolver(
                        e,
                        &resolver,
                        &effective_func_return_types,
                    )?),
                    location: loc.clone(),
                });
            }
            Statement::Continue => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: continue statement outside of function"
                    )]);
                }
                exec_statements.push(LocatedExecStatement {
                    statement: ExecStatement::Continue,
                    location: loc.clone(),
                });
            }
            Statement::Break => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: break statement outside of function"
                    )]);
                }
                exec_statements.push(LocatedExecStatement {
                    statement: ExecStatement::Break,
                    location: loc.clone(),
                });
            }
            Statement::While(expr, stat) => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: while statement outside of function"
                    )]);
                }
                let exec_cond = convert_to_exec_expression_with_resolver(
                    expr,
                    &resolver,
                    &effective_func_return_types,
                )?;
                // void 型の式は条件式に使用不可
                require_int_type(&exec_cond, &effective_func_return_types)?;
                let (s, es) = analyze_internal_with_parent(
                    stat,
                    ScopeType::Block,
                    Vec::new(),
                    Some(&resolver),
                    &mut Vec::new(),
                    &mut Vec::new(),
                    None,
                    effective_func_return_types.to_vec(),
                )?;
                exec_statements.push(LocatedExecStatement {
                    statement: ExecStatement::While(
                        ConditionMode::NonZero,
                        exec_cond,
                        Block {
                            scope: s.build(Vec::new(), Vec::new(), Vec::new()),
                            statements: es,
                        },
                    ),
                    location: loc.clone(),
                });
            }
            Statement::For(init_stmts, cond_stmts, step_stmts, body_stmts) => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: for statement outside of function"
                    )]);
                }

                // Step 1: init ブロックを解析（現在のスコープの子として）
                // init スコープには for ループ変数が含まれる
                let (init_sb, init_es) = analyze_internal_with_parent(
                    init_stmts,
                    ScopeType::Block,
                    Vec::new(),
                    Some(&resolver),
                    global_functions,
                    global_function_names,
                    None,
                    effective_func_return_types.to_vec(),
                )?;
                let init_scope = init_sb.build(Vec::new(), Vec::new(), Vec::new());

                // Step 2: for スコープのリゾルバを構築
                // 現在のスコープに init スコープを重ねることで、
                // cond/step/body から init 変数を scope_depth=1 でアクセス可能にする
                let mut for_resolver: ScopeResolver<'_> = ScopeResolver {
                    scope_stack: resolver.scope_stack.clone(),
                };
                // for-init スコープの constexpr は展開済みのため、空のテーブルを渡す
                let for_init_empty_constexpr: BTreeMap<String, i64> = BTreeMap::new();
                // for-init スコープの alias は展開済みのため、空のテーブルを渡す
                let for_init_empty_alias: BTreeMap<String, String> = BTreeMap::new();
                // for-init スコープのブロックエイリアスは展開済みのため、空のテーブルを渡す
                let for_init_empty_block_alias: BTreeMap<String, Vec<LocatedStatement>> = BTreeMap::new();
                for_resolver.enter_scope(
                    &init_scope.variable_indices,
                    &init_scope.variable_name_to_var_index,
                    &init_scope.variables,
                    &init_scope.identifier_map,
                    &for_init_empty_constexpr,
                    &for_init_empty_alias,
                    &for_init_empty_block_alias,
                    false,
                    None,
                );

                // Step 3: cond ブロックを解析
                let (cond_sb, cond_es) = analyze_internal_with_parent(
                    cond_stmts,
                    ScopeType::Block,
                    Vec::new(),
                    Some(&for_resolver),
                    &mut Vec::new(),
                    &mut Vec::new(),
                    None,
                    effective_func_return_types.to_vec(),
                )?;
                let cond_scope = cond_sb.build(Vec::new(), Vec::new(), Vec::new());

                // 条件ブロックの型チェック: 最後の式が int 型でなければならない
                let temp_cond_block = Block {
                    scope: cond_scope,
                    statements: cond_es,
                };
                if types::infer_block_type(&temp_cond_block, &effective_func_return_types)
                    != ValueType::Int
                {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: for condition block must end with an int-typed expression"
                    )]);
                }
                let Block {
                    scope: cond_scope,
                    statements: cond_es,
                } = temp_cond_block;

                // Step 4: step ブロックを解析
                let (step_sb, step_es) = analyze_internal_with_parent(
                    step_stmts,
                    ScopeType::Block,
                    Vec::new(),
                    Some(&for_resolver),
                    &mut Vec::new(),
                    &mut Vec::new(),
                    None,
                    effective_func_return_types.to_vec(),
                )?;

                // Step 5: body ブロックを解析
                let (body_sb, body_es) = analyze_internal_with_parent(
                    body_stmts,
                    ScopeType::Block,
                    Vec::new(),
                    Some(&for_resolver),
                    &mut Vec::new(),
                    &mut Vec::new(),
                    None,
                    effective_func_return_types.to_vec(),
                )?;

                exec_statements.push(LocatedExecStatement {
                    statement: ExecStatement::For(
                        Block {
                            scope: init_scope,
                            statements: init_es,
                        },
                        ConditionMode::NonZero,
                        Block {
                            scope: cond_scope,
                            statements: cond_es,
                        },
                        Block {
                            scope: step_sb.build(Vec::new(), Vec::new(), Vec::new()),
                            statements: step_es,
                        },
                        Block {
                            scope: body_sb.build(Vec::new(), Vec::new(), Vec::new()),
                            statements: body_es,
                        },
                    ),
                    location: loc.clone(),
                });
            }
            Statement::ConstexprDeclaration(_, _) => {
                // コンパイル時定数はパス0で処理済み。ExecStatement は生成しない
            }
            Statement::AliasIdentifier(_, _) => {
                // エイリアスはパス0で処理済み。ExecStatement は生成しない
            }
            Statement::AliasBlock(_, _) => {
                // ブロックエイリアスはパス0で処理済み。ExecStatement は生成しない
            }
            Statement::TemplateFunctionDefinition { .. } => {
                // テンプレート定義はプレパスで処理済み（expand_template_instantiations 参照）
                // ExecStatement は生成しない
            }
            Statement::AliasInstantiation { .. } => {
                // テンプレートインスタンス化はプレパスで FunctionDeclaration に展開済み
                // ExecStatement は生成しない
            }
            Statement::Invalid(_) => (),
        }
    }

    resolver.leave_scope();
    Ok((scope, exec_statements))
}

pub fn analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    // Phase 5: ルートの実行文（グローバル変数の初期化）も返す
    // Phase 5: global_functions と global_function_names を作成し、再帰呼び出しで共有
    let mut global_functions = Vec::new();
    let mut global_function_names = Vec::new();
    analyze_internal(
        root,
        ScopeType::Root,
        &mut global_functions,
        &mut global_function_names,
    )
    .map(|(scope, root_stmts)| scope.build(root_stmts, global_functions, global_function_names))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
