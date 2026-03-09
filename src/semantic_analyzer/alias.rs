//! # Alias 処理モジュール
//!
//! 識別子エイリアス・ブロックエイリアスの収集と検証を担当する。
//!
//! - 識別子エイリアス (`alias: name(target)`) の収集
//! - ブロックエイリアス (`alias: name { ... }`) の収集
//! - ブロックエイリアスの巡回参照検知（DFS）

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{Expression, LocatedStatement, Statement},
};

/// ステートメント列から alias（識別子エイリアス）定義を収集し、
/// エイリアステーブル `BTreeMap<String, String>` を返す。
///
/// 重複定義はエラーとして報告する。名前空間内のエイリアスはマングル名で登録される。
pub(super) fn collect_alias_map(
    statements: &[LocatedStatement],
) -> Result<BTreeMap<String, String>, Vec<CodeParseError>> {
    let mut alias_map: BTreeMap<String, String> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    collect_alias_map_recursive(statements, "", &mut alias_map, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(alias_map)
}

/// 再帰的に alias を収集するヘルパー
fn collect_alias_map_recursive(
    statements: &[LocatedStatement],
    ns_prefix: &str,
    alias_map: &mut BTreeMap<String, String>,
    errors: &mut Vec<CodeParseError>,
) {
    for located_stat in statements {
        match &located_stat.statement {
            Statement::AliasIdentifier(name, target) => {
                let mangled = format!("{}{}", ns_prefix, name);
                if alias_map.contains_key(&mangled) {
                    errors.push(code_parse_error!(
                        located_stat.location.start,
                        format!("duplicate alias definition: '{}'", mangled)
                    ));
                } else {
                    alias_map.insert(mangled, target.clone());
                }
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                collect_alias_map_recursive(body, &sub_prefix, alias_map, errors);
            }
            _ => {}
        }
    }
}

/// ステートメント列からブロックエイリアス定義を収集し、
/// ブロックエイリアステーブル `BTreeMap<String, Vec<LocatedStatement>>` を返す。
///
/// 重複定義・識別子エイリアスとの名前衝突はエラーとして報告する。
/// 名前空間内のブロックエイリアスはマングル名で登録される。
pub(super) fn collect_block_alias_map(
    statements: &[LocatedStatement],
    alias_map: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Vec<LocatedStatement>>, Vec<CodeParseError>> {
    let mut block_alias_map: BTreeMap<String, Vec<LocatedStatement>> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    collect_block_alias_map_recursive(statements, "", alias_map, &mut block_alias_map, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(block_alias_map)
}

/// 再帰的にブロックエイリアスを収集するヘルパー
fn collect_block_alias_map_recursive(
    statements: &[LocatedStatement],
    ns_prefix: &str,
    alias_map: &BTreeMap<String, String>,
    block_alias_map: &mut BTreeMap<String, Vec<LocatedStatement>>,
    errors: &mut Vec<CodeParseError>,
) {
    for located_stat in statements {
        match &located_stat.statement {
            Statement::AliasBlock(name, body) => {
                let mangled = format!("{}{}", ns_prefix, name);
                if block_alias_map.contains_key(&mangled) {
                    errors.push(code_parse_error!(
                        located_stat.location.start,
                        format!("duplicate block alias definition: '{}'", mangled)
                    ));
                } else if alias_map.contains_key(&mangled) {
                    errors.push(code_parse_error!(
                        located_stat.location.start,
                        format!(
                            "alias '{}' is defined as both identifier alias and block alias",
                            mangled
                        )
                    ));
                } else {
                    block_alias_map.insert(mangled, body.clone());
                }
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                collect_block_alias_map_recursive(body, &sub_prefix, alias_map, block_alias_map, errors);
            }
            _ => {}
        }
    }
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
    expr: &crate::tree_parser::LocatedExpression,
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
pub(super) fn detect_block_alias_cycles(
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
