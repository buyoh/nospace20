//! # テンプレート展開モジュール
//!
//! テンプレート関数のインスタンス化を担当する。
//!
//! - `TemplateFunctionDefinition` からのテンプレート定義収集
//! - `AliasInstantiation` を `FunctionDeclaration` へ展開
//! - エイリアスパラメータ数の検証

use std::collections::BTreeMap;

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{
        AliasArg, AliasParam, AliasParamKind, Expression, LocatedExpression, LocatedStatement,
        Statement,
    },
};

/// テンプレートエントリ（`TemplateFunctionDefinition` から収集）
struct TemplateEntry {
    args: Vec<String>,
    alias_params: Vec<AliasParam>,
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
pub(super) fn expand_template_instantiations(
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
