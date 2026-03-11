//! # Constexpr 評価モジュール
//!
//! コンパイル時定数 (`constexpr`) の収集と評価を担当する。
//!
//! - 式の再帰評価
//! - 名前による遅延解決（巡回検知付き）
//! - ステートメント列からの constexpr テーブル構築

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    base::{constexpr_eval, pure_eval, CodeParseError},
    code_parse_error,
    tree_parser::{
        Expression, LocatedExpression, LocatedStatement, Operator1, Operator2, Statement,
    },
};

fn ns_candidate_names(current_ns: &str, name: &str) -> Vec<String> {
    if current_ns.is_empty() {
        return vec![name.to_string()];
    }

    let parts: Vec<&str> = current_ns.split('$').collect();
    let mut candidates = Vec::new();
    for i in (0..=parts.len()).rev() {
        if i == 0 {
            candidates.push(name.to_string());
        } else {
            candidates.push(format!("{}${}", parts[..i].join("$"), name));
        }
    }
    candidates
}

/// constexpr 式を再帰的に評価する
///
/// `raw` は未解決の constexpr 定義（名前 → 生式）。
/// `resolved` は解決済みの constexpr 定数テーブル（名前 → 値）。
/// `evaluating` は巡回参照検知用の「現在評価中」セット。
fn evaluate_constexpr_expr(
    expr: &LocatedExpression,
    current_ns: &str,
    raw: &BTreeMap<String, Box<LocatedExpression>>,
    import_table: &BTreeMap<String, BTreeMap<String, String>>,
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
                return evaluate_constexpr_by_name(
                    name,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                );
            }
            if !name.contains('$') {
                for candidate in ns_candidate_names(current_ns, name) {
                    if let Some(&v) = resolved.get(candidate.as_str()) {
                        return Ok(v);
                    }
                    if raw.contains_key(candidate.as_str()) {
                        return evaluate_constexpr_by_name(
                            candidate.as_str(),
                            raw,
                            import_table,
                            resolved,
                            evaluating,
                        );
                    }
                }

                if let Some(imported_name) = import_table
                    .get(current_ns)
                    .and_then(|m| m.get(name.as_str()))
                    .cloned()
                {
                    if let Some(&v) = resolved.get(imported_name.as_str()) {
                        return Ok(v);
                    }
                    if raw.contains_key(imported_name.as_str()) {
                        return evaluate_constexpr_by_name(
                            imported_name.as_str(),
                            raw,
                            import_table,
                            resolved,
                            evaluating,
                        );
                    }
                }
            }
            Err(vec![code_parse_error!(
                loc,
                format!("'{}' is not a compile-time constant", name)
            )])
        }
        Expression::Operation1(op, inner) => match op {
            Operator1::Negative => {
                let v = evaluate_constexpr_expr(
                    inner,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
                Ok(v.wrapping_neg())
            }
            Operator1::LogicalNot => {
                let v = evaluate_constexpr_expr(
                    inner,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
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
                let lv = evaluate_constexpr_expr(
                    l,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
                if lv == 0 {
                    return Ok(0);
                }
                let rv = evaluate_constexpr_expr(
                    r,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
                Ok(pure_eval::bool_to_int(rv != 0))
            }
            Operator2::LogicalOr => {
                // 短絡評価: 左辺が非0なら右辺を評価しない
                let lv = evaluate_constexpr_expr(
                    l,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
                if lv != 0 {
                    return Ok(1);
                }
                let rv = evaluate_constexpr_expr(
                    r,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
                Ok(pure_eval::bool_to_int(rv != 0))
            }
            _ => {
                // Plus, Minus, Multiply, Divide, Modulo, 比較演算
                let lv = evaluate_constexpr_expr(
                    l,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
                let rv = evaluate_constexpr_expr(
                    r,
                    current_ns,
                    raw,
                    import_table,
                    resolved,
                    evaluating,
                )?;
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
    import_table: &BTreeMap<String, BTreeMap<String, String>>,
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
    let current_ns = name
        .rsplit_once('$')
        .map(|(ns, _)| ns.to_string())
        .unwrap_or_default();
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
            evaluate_constexpr_expr(
                &expr,
                current_ns.as_str(),
                raw,
                import_table,
                resolved,
                evaluating,
            )?
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
pub(super) fn collect_constexpr_table(
    statements: &[LocatedStatement],
    import_table: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<BTreeMap<String, i64>, Vec<CodeParseError>> {
    // 全ての constexpr を収集（名前空間内のものをマングル名で含む）
    let mut raw: BTreeMap<String, Box<LocatedExpression>> = BTreeMap::new();
    collect_constexpr_raw(statements, "", &mut raw);

    if raw.is_empty() {
        return Ok(BTreeMap::new());
    }

    // 各 constexpr を遅延評価
    let mut resolved: BTreeMap<String, i64> = BTreeMap::new();
    let mut errors: Vec<CodeParseError> = Vec::new();
    for name in raw.keys().cloned().collect::<Vec<_>>() {
        let mut evaluating: BTreeSet<String> = BTreeSet::new();
        match evaluate_constexpr_by_name(
            &name,
            &raw,
            import_table,
            &mut resolved,
            &mut evaluating,
        ) {
            Ok(_) => {}
            Err(mut errs) => errors.append(&mut errs),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(resolved)
}

/// 名前空間を再帰的に探索し、constexpr の生式マップに収集する
///
/// `ns_prefix` が空の場合は名前をそのまま、非空の場合は `{ns_prefix}{name}` でマングル
fn collect_constexpr_raw(
    statements: &[LocatedStatement],
    ns_prefix: &str,
    raw: &mut BTreeMap<String, Box<LocatedExpression>>,
) {
    for located_stat in statements {
        match &located_stat.statement {
            Statement::ConstexprDeclaration(name, expr) => {
                let mangled = format!("{}{}", ns_prefix, name);
                raw.insert(mangled, expr.clone());
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                collect_constexpr_raw(body, &sub_prefix, raw);
            }
            _ => {}
        }
    }
}
