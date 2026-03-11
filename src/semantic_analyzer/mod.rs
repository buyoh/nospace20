//! # Semantic Analyzer
//!
//! 意味解析器。ASTを実行可能な構造に変換する。
//!
//! ## モジュール構成
//!
//! - `context`    : 解析コンテキスト（AnalyzeContext 構造体）
//! - `expression` : 式の変換処理（ExecExpression 生成）
//! - `statement`  : 文の変換処理（ExecStatement 生成）
//! - `scope`      : スコープ・識別子解決
//! - `types`      : 中間表現の型定義
//! - `alias`      : エイリアス収集・解決
//! - `constexpr`  : コンパイル時定数
//! - `template`   : テンプレート展開
//! - `return_analysis` : return 文制御フロー解析

mod alias;
mod constexpr;
mod context;
mod expression;
mod return_analysis;
mod scope;
mod statement;
mod template;
mod types;

use std::collections::{BTreeMap, BTreeSet};

use alias::{collect_alias_map, collect_block_alias_map, detect_block_alias_cycles};
use constexpr::collect_constexpr_table;
use return_analysis::{guarantees_return, has_return_statement};
use scope::{FunctionIndex, Identifier, ScopeBuilder, ScopeResolver, ScopeType, SymbolTable};
use template::expand_template_instantiations;

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{LocatedStatement, Statement, StructFieldDecl, TypeSpec},
};

// tests.rs が `use super::*;` でこれらの型を使用するため、テスト時のみインポートする
#[cfg(test)]
#[allow(unused_imports)]
use crate::tree_parser::{Expression, LocatedExpression, Operator1, Operator2};

pub use scope::{Function, Scope};
pub(crate) use types::{
    Block, ConditionMode, ExecExpression, ExecStatement, InternalBuiltinFunctionKind,
    LocatedExecExpression, LocatedExecStatement, Variable,
};
pub use types::{BuiltinFunctionKind, IdentifierRef, ValueType};

/// ブロック式の解析に使用するヘルパー関数。
///
/// 式 (Expression) の中でブロックスコープが出現する場合（If/Block/ブロックエイリアス呼び出し等）
/// に `analyze_internal_with_parent` を呼び出すための簡易ラッパー。
///
/// ブロック式の中では新しい関数宣言は許可されないため、
/// グローバル関数リストへの参照として一時的な空 Vec を使用する。
fn analyze_block_for_expression(
    statements: &Vec<LocatedStatement>,
    parent_resolver: &ScopeResolver,
    func_return_types: &[ValueType],
) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>> {
    let mut temp_global_functions = Vec::new();
    let mut temp_global_function_names = Vec::new();
    let mut ctx = context::AnalyzeContext {
        global_functions: &mut temp_global_functions,
        global_function_names: &mut temp_global_function_names,
        func_global_index: None,
        inherited_func_return_types: func_return_types.to_vec(),
    };
    analyze_internal_with_parent(
        statements,
        ScopeType::Block,
        Vec::new(),
        Some(parent_resolver),
        &mut ctx,
    )
}

fn analyze_internal(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    ctx: &mut context::AnalyzeContext,
) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>> {
    analyze_internal_with_parent(statements, scope_type, Vec::new(), None, ctx)
}

/// 初期変数と親のリゾルバを指定して解析する
///
/// - `statements`      : 解析対象の文リスト
/// - `scope_type`      : スコープ種別（Root / Function / Block）
/// - `initial_vars`    : 事前登録する変数名（関数の引数など）
/// - `parent_resolver` : 親スコープのリゾルバ（None = ルートスコープ）
/// - `ctx`             : 解析コンテキスト
fn analyze_internal_with_parent(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    initial_vars: Vec<(String, Option<TypeSpec>)>,
    parent_resolver: Option<&ScopeResolver>,
    ctx: &mut context::AnalyzeContext,
) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>> {
    // テンプレート関数のインスタンス化を展開するプレパス
    // TemplateFunctionDefinition と AliasInstantiation を FunctionDeclaration に変換する
    let expanded_statements = expand_template_instantiations(statements)?;
    let statements: &Vec<LocatedStatement> = &expanded_statements;

    let mut scope = ScopeBuilder::new();

    if let Some(parent) = parent_resolver {
        if let Some(parent_scope) = parent.scope_stack.last() {
            scope.struct_definitions = parent_scope.struct_definitions.clone();
            scope.struct_name_to_index = parent_scope.struct_name_to_index.clone();
        }
    }

    // グローバル変数は暗黙的に static
    let is_static = matches!(scope_type, ScopeType::Root);
    let is_function_scope = matches!(scope_type, ScopeType::Root | ScopeType::Function);

    // 初期変数を登録（関数の引数など）
    for (var_name, type_annot) in initial_vars {
        let value_type = match type_annot {
            None => ValueType::Int,
            Some(spec) => match spec {
                TypeSpec::Int => ValueType::Int,
                _ => {
                    return Err(vec![code_parse_error!(format!(
                        "semantic error: argument '{}' must be int type",
                        var_name
                    ))]);
                }
            },
        };
        scope.add_variable(
            &var_name,
            Variable {
                slot_index: 0,
                is_static: false,
                array_size: None,
                is_final: false,
                value_type,
            },
        )?;
    }

    // 3パス解析 → 4パス解析（Pass 0 を追加）
    let mut import_bundle = collect_import_bundle(statements)?;
    if let Some(parent) = parent_resolver {
        for (ns, member_map) in &parent.import_table {
            let entry = import_bundle.import_table.entry(ns.clone()).or_default();
            for (member, target) in member_map {
                entry
                    .entry(member.clone())
                    .or_insert_with(|| target.clone());
            }
        }
    }

    // パス0: constexpr 定義の収集・評価
    let constexpr_table_temp = collect_constexpr_table(statements, &import_bundle.import_table)?;
    // パス0: alias（識別子エイリアス）定義の収集
    let mut alias_map_temp = collect_alias_map(statements)?;
    for (k, v) in import_bundle.export_aliases {
        if alias_map_temp.contains_key(&k) {
            return Err(vec![code_parse_error!(format!(
                "semantic error: duplicate alias definition '{}'",
                k
            ))]);
        }
        alias_map_temp.insert(k, v);
    }
    // パス0: ブロックエイリアス定義の収集
    let block_alias_map_temp = collect_block_alias_map(statements, &alias_map_temp)?;
    // パス0: ブロックエイリアスの巡回参照チェック
    detect_block_alias_cycles(&block_alias_map_temp, &alias_map_temp)?;

    // パス1a: 構造体定義を収集
    collect_struct_declarations(statements, "", &mut scope)?;

    // パス1b: 関数宣言を先にスキャンして登録（ホイスティング対応）
    // 名前空間内の関数もマングル名で登録する
    scan_function_declarations(statements, "", &mut scope, ctx)?;

    // 型チェック用の関数戻り値型スライスを決定
    // inherited_func_return_types が空 = ルートまたは関数スコープ → global_functions から収集
    // inherited_func_return_types が非空 = if/while/block の内部 → 外側の型コンテキストを継承
    let effective_func_return_types: Vec<ValueType> = if ctx.inherited_func_return_types.is_empty()
    {
        ctx.global_functions
            .iter()
            .map(|f| f.return_type.clone())
            .collect()
    } else {
        ctx.inherited_func_return_types.clone()
    };

    // パス1c: 変数宣言収集（ホイスティング対応）
    // 名前空間内の変数もマングル名で登録する
    scan_variable_declarations(statements, "", is_static, &mut scope)?;

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
    // identifier_map も保持して関数解決に使用
    let temporary_scope = Scope {
        identifier_map: scope.identifier_map.clone(), // 関数解決に必要
        variable_indices: variable_indices_temp.clone(),
        variable_name_to_var_index: variable_name_to_var_index_temp.clone(),
        variables: scope.variables.clone(), // Clone が必要
        variable_count: slot_index,
        functions: Vec::new(), // 未使用
        struct_definitions: scope.struct_definitions.clone(),
        struct_name_to_index: scope.struct_name_to_index.clone(),
        symbol_table: SymbolTable {
            function_names: Vec::new(),
            function_name_to_index: BTreeMap::new(),
        },
        main_function_index: None,          // 一時スコープなので None
        static_init_statements: Vec::new(), // 未使用
        root_statements: Vec::new(),        // 未使用
    };

    // 親のリゾルバを継承して新しいリゾルバを作成
    let mut resolver = if let Some(parent) = parent_resolver {
        let mut new_resolver = ScopeResolver {
            scope_stack: parent.scope_stack.clone(),
            namespace_prefix: parent.namespace_prefix.clone(),
            import_table: import_bundle.import_table.clone(),
        };
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variable_name_to_var_index,
            &temporary_scope.variables,
            &temporary_scope.identifier_map,
            &constexpr_table_temp,
            &alias_map_temp,
            &block_alias_map_temp,
            &temporary_scope.struct_definitions,
            &temporary_scope.struct_name_to_index,
            is_function_scope,
            ctx.func_global_index,
        );
        new_resolver
    } else {
        let mut new_resolver = ScopeResolver::new();
        new_resolver.import_table = import_bundle.import_table.clone();
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variable_name_to_var_index,
            &temporary_scope.variables,
            &temporary_scope.identifier_map,
            &constexpr_table_temp,
            &alias_map_temp,
            &block_alias_map_temp,
            &temporary_scope.struct_definitions,
            &temporary_scope.struct_name_to_index,
            is_function_scope,
            ctx.func_global_index,
        );
        new_resolver
    };

    // パス2: 文の変換（statement モジュールに委譲）
    let exec_statements = statement::convert_to_exec_statements(
        statements,
        scope_type,
        &mut scope,
        &resolver,
        &effective_func_return_types,
        ctx,
    )?;

    resolver.leave_scope();
    Ok((scope, exec_statements))
}

pub fn analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    let mut global_functions = Vec::new();
    let mut global_function_names = Vec::new();
    let mut ctx =
        context::AnalyzeContext::new_root(&mut global_functions, &mut global_function_names);
    analyze_internal(root, ScopeType::Root, &mut ctx)
        .map(|(scope, root_stmts)| scope.build(root_stmts, global_functions, global_function_names))
}

#[derive(Clone)]
struct ImportDecl {
    current_ns: String,
    target_ns_name: String,
    is_weak: bool,
    is_export: bool,
    location: usize,
}

struct ImportBundle {
    import_table: BTreeMap<String, BTreeMap<String, String>>,
    export_aliases: BTreeMap<String, String>,
}

fn ns_prefix(ns: &str) -> String {
    if ns.is_empty() {
        String::new()
    } else {
        format!("{}$", ns)
    }
}

fn add_direct_member(members: &mut BTreeMap<String, BTreeSet<String>>, ns: &str, name: &str) {
    members
        .entry(ns.to_string())
        .or_default()
        .insert(name.to_string());
}

fn collect_namespace_info_recursive(
    statements: &[LocatedStatement],
    current_ns: &str,
    known_namespaces: &mut BTreeSet<String>,
    direct_members: &mut BTreeMap<String, BTreeSet<String>>,
    imports: &mut Vec<ImportDecl>,
) {
    for located in statements {
        match &located.statement {
            Statement::VariableDeclaration(name, _, _, _, _, _) => {
                add_direct_member(direct_members, current_ns, name);
            }
            Statement::FunctionDeclaration(name, _, _, _) => {
                add_direct_member(direct_members, current_ns, name);
            }
            Statement::ConstexprDeclaration(name, _) => {
                add_direct_member(direct_members, current_ns, name);
            }
            Statement::AliasIdentifier(name, _) | Statement::AliasBlock(name, _) => {
                add_direct_member(direct_members, current_ns, name);
            }
            Statement::ImportDeclaration {
                namespace_name,
                is_weak,
                is_export,
            } => {
                imports.push(ImportDecl {
                    current_ns: current_ns.to_string(),
                    target_ns_name: namespace_name.clone(),
                    is_weak: *is_weak,
                    is_export: *is_export,
                    location: located.location.start,
                });
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_ns = if current_ns.is_empty() {
                    ns_name.clone()
                } else {
                    format!("{}${}", current_ns, ns_name)
                };
                known_namespaces.insert(sub_ns.clone());
                collect_namespace_info_recursive(
                    body,
                    &sub_ns,
                    known_namespaces,
                    direct_members,
                    imports,
                );
            }
            _ => {}
        }
    }
}

fn resolve_namespace_name(
    current_ns: &str,
    target: &str,
    known_namespaces: &BTreeSet<String>,
) -> Option<String> {
    if target.contains('$') {
        return known_namespaces
            .contains(target)
            .then(|| target.to_string());
    }

    if current_ns.is_empty() {
        return known_namespaces
            .contains(target)
            .then(|| target.to_string());
    }

    let parts: Vec<&str> = current_ns.split('$').collect();
    for i in (0..=parts.len()).rev() {
        let candidate = if i == 0 {
            target.to_string()
        } else {
            format!("{}${}", parts[..i].join("$"), target)
        };
        if known_namespaces.contains(candidate.as_str()) {
            return Some(candidate);
        }
    }
    None
}

fn collect_import_bundle(
    statements: &[LocatedStatement],
) -> Result<ImportBundle, Vec<CodeParseError>> {
    let mut known_namespaces: BTreeSet<String> = BTreeSet::new();
    let mut direct_members: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut imports: Vec<ImportDecl> = Vec::new();

    collect_namespace_info_recursive(
        statements,
        "",
        &mut known_namespaces,
        &mut direct_members,
        &mut imports,
    );

    let mut import_table: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut export_aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut seen_imports: BTreeSet<(String, String)> = BTreeSet::new();
    let mut errors: Vec<CodeParseError> = Vec::new();

    for imp in imports {
        let Some(resolved_target_ns) = resolve_namespace_name(
            imp.current_ns.as_str(),
            imp.target_ns_name.as_str(),
            &known_namespaces,
        ) else {
            errors.push(code_parse_error!(
                imp.location,
                format!(
                    "semantic error: undefined namespace '{}'",
                    imp.target_ns_name
                )
            ));
            continue;
        };

        if imp.current_ns == resolved_target_ns {
            errors.push(code_parse_error!(
                imp.location,
                format!(
                    "semantic error: cannot import current namespace '{}'",
                    imp.target_ns_name
                )
            ));
            continue;
        }

        let import_key = (imp.current_ns.clone(), resolved_target_ns.clone());
        if seen_imports.contains(&import_key) {
            errors.push(code_parse_error!(
                imp.location,
                format!(
                    "semantic error: duplicate import of namespace '{}'",
                    imp.target_ns_name
                )
            ));
            continue;
        }
        seen_imports.insert(import_key);

        let source_members = direct_members
            .get(resolved_target_ns.as_str())
            .cloned()
            .unwrap_or_default();
        let local_members = direct_members
            .get(imp.current_ns.as_str())
            .cloned()
            .unwrap_or_default();

        for member in source_members {
            let imported_full = format!("{}{}", ns_prefix(&resolved_target_ns), member);

            if local_members.contains(member.as_str()) {
                if imp.is_weak {
                    continue;
                }
                errors.push(code_parse_error!(
                    imp.location,
                    format!(
                        "semantic error: imported identifier '{}' conflicts with existing declaration",
                        member
                    )
                ));
                continue;
            }

            let local_table = import_table.entry(imp.current_ns.clone()).or_default();
            if let Some(existing) = local_table.get(member.as_str()) {
                if existing != &imported_full {
                    if imp.is_weak {
                        continue;
                    }
                    errors.push(code_parse_error!(
                        imp.location,
                        format!(
                            "semantic error: imported identifier '{}' conflicts with another import",
                            member
                        )
                    ));
                }
                continue;
            }
            local_table.insert(member.clone(), imported_full.clone());

            if imp.is_export {
                let exported_key = format!("{}{}", ns_prefix(&imp.current_ns), member);
                if let Some(existing) = export_aliases.get(exported_key.as_str()) {
                    if existing != &imported_full {
                        errors.push(code_parse_error!(
                            imp.location,
                            format!(
                                "semantic error: exported import '{}' conflicts with another export",
                                exported_key
                            )
                        ));
                    }
                    continue;
                }
                export_aliases.insert(exported_key, imported_full);
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ImportBundle {
        import_table,
        export_aliases,
    })
}

/// パス1a: 関数宣言を再帰的にスキャンしてスコープビルダーに登録する（名前空間対応）
///
/// 名前空間内の関数は `{ns_prefix}{name}` でマングルされて登録される。
fn scan_function_declarations(
    statements: &[LocatedStatement],
    ns_prefix: &str,
    scope: &mut ScopeBuilder,
    ctx: &mut context::AnalyzeContext,
) -> Result<(), Vec<CodeParseError>> {
    for located_stat in statements {
        match &located_stat.statement {
            Statement::FunctionDeclaration(name, args, body, return_type_annot) => {
                let mangled_name = format!("{}{}", ns_prefix, name);
                let global_idx = ctx.global_functions.len();

                for (arg_name, type_annot) in args {
                    if let Some(spec) = type_annot {
                        if !matches!(spec, TypeSpec::Int) {
                            return Err(vec![code_parse_error!(
                                located_stat.location.start,
                                format!("semantic error: argument '{}' must be int type", arg_name)
                            )]);
                        }
                    }
                }

                let has_ret = has_return_statement(body);
                let return_type = if let Some(spec) = return_type_annot {
                    match spec {
                        TypeSpec::Int => {
                            if has_ret && !guarantees_return(body) {
                                return Err(vec![code_parse_error!(format!(
                                    "semantic error: function '{}' has mixed return types (return in some paths but not all)",
                                    mangled_name
                                ))]);
                            }
                            if !has_ret {
                                return Err(vec![code_parse_error!(
                                    located_stat.location.start,
                                    format!(
                                        "semantic error: function '{}' has @int but no return value",
                                        mangled_name
                                    )
                                )]);
                            }
                            ValueType::Int
                        }
                        TypeSpec::Void => {
                            if has_ret {
                                return Err(vec![code_parse_error!(
                                    located_stat.location.start,
                                    format!(
                                        "semantic error: function '{}' has @void but returns a value",
                                        mangled_name
                                    )
                                )]);
                            }
                            ValueType::Void
                        }
                        _ => {
                            return Err(vec![code_parse_error!(
                                located_stat.location.start,
                                format!(
                                    "semantic error: function '{}' has unsupported return type",
                                    mangled_name
                                )
                            )]);
                        }
                    }
                } else {
                    if has_ret && !guarantees_return(body) {
                        return Err(vec![code_parse_error!(format!(
                            "semantic error: function '{}' has mixed return types (return in some paths but not all)",
                            mangled_name
                        ))]);
                    }
                    if has_ret {
                        ValueType::Int
                    } else {
                        ValueType::Void
                    }
                };

                ctx.global_function_names.push(mangled_name.clone());
                ctx.global_functions.push(Function {
                    arg_indices: Vec::new(),
                    return_type: return_type.clone(),
                    is_unused: false,
                    block: Block {
                        scope: Scope {
                            identifier_map: BTreeMap::new(),
                            variable_indices: BTreeMap::new(),
                            variable_name_to_var_index: BTreeMap::new(),
                            variables: Vec::new(),
                            variable_count: 0,
                            functions: Vec::new(),
                            struct_definitions: Vec::new(),
                            struct_name_to_index: BTreeMap::new(),
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
                    &mangled_name,
                    Identifier::Function(FunctionIndex(global_idx, args.len(), return_type)),
                )?;
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                scan_function_declarations(body, &sub_prefix, scope, ctx)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// パス1b: 変数宣言を再帰的にスキャンしてスコープビルダーに登録する（名前空間対応）
///
/// 名前空間内の変数は `{ns_prefix}{name}` でマングルされて登録される。
fn scan_variable_declarations(
    statements: &[LocatedStatement],
    ns_prefix: &str,
    is_static: bool,
    scope: &mut ScopeBuilder,
) -> Result<(), Vec<CodeParseError>> {
    for located_stat in statements {
        match &located_stat.statement {
            Statement::VariableDeclaration(
                name,
                _,
                is_static_explicit,
                is_final,
                array_size,
                type_annot,
            ) => {
                let mangled_name = format!("{}{}", ns_prefix, name);
                // グローバル変数は暗黙的に static、明示的 static も考慮
                let final_is_static = *is_static_explicit || is_static;
                let mut effective_array_size = array_size.map(|n| n as usize);
                let value_type = if let Some(spec) = type_annot {
                    resolve_value_type(spec, scope)?
                } else if let Some(size) = effective_array_size {
                    ValueType::Array(Box::new(ValueType::Int), size)
                } else {
                    ValueType::Int
                };

                if matches!(value_type, ValueType::Void) {
                    return Err(vec![code_parse_error!(
                        located_stat.location.start,
                        format!(
                            "semantic error: variable '{}' cannot have void type",
                            mangled_name
                        )
                    )]);
                }

                if let ValueType::Struct(idx) = value_type {
                    let def = scope.struct_definitions.get(idx).ok_or_else(|| {
                        vec![code_parse_error!(
                            located_stat.location.start,
                            format!("semantic error: unknown struct type for '{}'", mangled_name)
                        )]
                    })?;
                    effective_array_size = Some(def.total_size);
                } else if let ValueType::Array(_, size) = &value_type {
                    effective_array_size = Some(*size);
                }

                scope.add_variable(
                    &mangled_name,
                    Variable {
                        slot_index: 0, // build() で正しい値に設定される
                        is_static: final_is_static,
                        array_size: effective_array_size,
                        is_final: *is_final,
                        value_type,
                    },
                )?;
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                scan_variable_declarations(body, &sub_prefix, is_static, scope)?;
            }
            // 以下はパス0で処理済み
            Statement::FunctionDeclaration(_, _, _, _)
            | Statement::StructDeclaration(_, _)
            | Statement::ConstexprDeclaration(_, _)
            | Statement::AliasIdentifier(_, _)
            | Statement::AliasBlock(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}

fn resolve_value_type(
    type_spec: &TypeSpec,
    scope: &ScopeBuilder,
) -> Result<ValueType, Vec<CodeParseError>> {
    match type_spec {
        TypeSpec::Int => Ok(ValueType::Int),
        TypeSpec::Void => Ok(ValueType::Void),
        TypeSpec::Named(name) => scope
            .struct_name_to_index
            .get(name)
            .map(|idx| ValueType::Struct(*idx))
            .ok_or_else(|| {
                vec![code_parse_error!(format!(
                    "semantic error: undefined struct type '{}'",
                    name
                ))]
            }),
        TypeSpec::Array(inner, size) => {
            let inner_value = resolve_value_type(inner, scope)?;
            Ok(ValueType::Array(Box::new(inner_value), *size))
        }
        TypeSpec::Ref(_) => Ok(ValueType::Int),
    }
}

fn collect_struct_declarations(
    statements: &[LocatedStatement],
    ns_prefix: &str,
    scope: &mut ScopeBuilder,
) -> Result<(), Vec<CodeParseError>> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut field_map: BTreeMap<String, Vec<StructFieldDecl>> = BTreeMap::new();
    let mut errors = Vec::new();

    collect_struct_declarations_recursive(
        statements,
        ns_prefix,
        scope,
        &mut field_map,
        &mut errors,
    );

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut visiting = BTreeSet::new();
    let mut resolved = BTreeSet::new();
    let names: Vec<String> = field_map.keys().cloned().collect();
    for name in names {
        resolve_struct_definition(
            &name,
            scope,
            &field_map,
            &mut visiting,
            &mut resolved,
            &mut errors,
        );
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(())
}

fn collect_struct_declarations_recursive(
    statements: &[LocatedStatement],
    ns_prefix: &str,
    scope: &mut ScopeBuilder,
    field_map: &mut std::collections::BTreeMap<String, Vec<StructFieldDecl>>,
    errors: &mut Vec<CodeParseError>,
) {
    for located_stat in statements {
        match &located_stat.statement {
            Statement::StructDeclaration(name, fields) => {
                let mangled = format!("{}{}", ns_prefix, name);
                if !name
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
                {
                    errors.push(code_parse_error!(
                        located_stat.location.start,
                        "struct name must start with an uppercase letter"
                    ));
                    continue;
                }
                if scope.struct_name_to_index.contains_key(&mangled) {
                    errors.push(code_parse_error!(
                        located_stat.location.start,
                        format!("semantic error: duplicate struct definition '{}'", mangled)
                    ));
                    continue;
                }
                let idx = scope.struct_definitions.len();
                scope.struct_name_to_index.insert(mangled.clone(), idx);
                scope.struct_definitions.push(types::StructDefinition {
                    name: mangled.clone(),
                    fields: Vec::new(),
                    total_size: 0,
                });
                field_map.insert(mangled, fields.clone());
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                collect_struct_declarations_recursive(body, &sub_prefix, scope, field_map, errors);
            }
            _ => {}
        }
    }
}

fn resolve_struct_definition(
    name: &str,
    scope: &mut ScopeBuilder,
    field_map: &std::collections::BTreeMap<String, Vec<StructFieldDecl>>,
    visiting: &mut std::collections::BTreeSet<String>,
    resolved: &mut std::collections::BTreeSet<String>,
    errors: &mut Vec<CodeParseError>,
) {
    if resolved.contains(name) {
        return;
    }
    if visiting.contains(name) {
        errors.push(code_parse_error!(format!(
            "semantic error: recursive struct definition '{}'",
            name
        )));
        return;
    }

    visiting.insert(name.to_string());

    let fields = match field_map.get(name) {
        Some(f) => f,
        None => {
            errors.push(code_parse_error!(format!(
                "semantic error: missing struct definition '{}'",
                name
            )));
            visiting.remove(name);
            return;
        }
    };

    let mut resolved_fields = Vec::new();
    let mut offset = 0usize;

    for field in fields {
        let value_type = if let Some(spec) = &field.type_spec {
            match resolve_type_spec_with_structs(spec, scope, field_map, visiting, resolved, errors)
            {
                Some(v) => v,
                None => {
                    visiting.remove(name);
                    return;
                }
            }
        } else if let Some(size) = field.array_size {
            ValueType::Array(Box::new(ValueType::Int), size)
        } else {
            ValueType::Int
        };

        if matches!(value_type, ValueType::Void) {
            errors.push(code_parse_error!(format!(
                "semantic error: field '{}' in '{}' cannot be void",
                field.name, name
            )));
            visiting.remove(name);
            return;
        }

        let size = value_type_size(&value_type, scope, field_map, visiting, resolved, errors);
        if size == 0 {
            visiting.remove(name);
            return;
        }

        resolved_fields.push(types::StructField {
            name: field.name.clone(),
            value_type: value_type.clone(),
            offset,
            size,
        });
        offset += size;
    }

    if let Some(idx) = scope.struct_name_to_index.get(name).cloned() {
        if let Some(def) = scope.struct_definitions.get_mut(idx) {
            def.fields = resolved_fields;
            def.total_size = offset;
        }
    }

    visiting.remove(name);
    resolved.insert(name.to_string());
}

fn resolve_type_spec_with_structs(
    spec: &TypeSpec,
    scope: &mut ScopeBuilder,
    field_map: &std::collections::BTreeMap<String, Vec<StructFieldDecl>>,
    visiting: &mut std::collections::BTreeSet<String>,
    resolved: &mut std::collections::BTreeSet<String>,
    errors: &mut Vec<CodeParseError>,
) -> Option<ValueType> {
    match spec {
        TypeSpec::Int => Some(ValueType::Int),
        TypeSpec::Void => Some(ValueType::Void),
        TypeSpec::Named(name) => {
            if !scope.struct_name_to_index.contains_key(name) {
                errors.push(code_parse_error!(format!(
                    "semantic error: undefined struct type '{}'",
                    name
                )));
                return None;
            }
            resolve_struct_definition(name, scope, field_map, visiting, resolved, errors);
            scope
                .struct_name_to_index
                .get(name)
                .cloned()
                .map(ValueType::Struct)
        }
        TypeSpec::Array(inner, size) => {
            let inner_value = resolve_type_spec_with_structs(
                inner, scope, field_map, visiting, resolved, errors,
            )?;
            Some(ValueType::Array(Box::new(inner_value), *size))
        }
        TypeSpec::Ref(_) => Some(ValueType::Int),
    }
}

fn value_type_size(
    value_type: &ValueType,
    scope: &mut ScopeBuilder,
    field_map: &std::collections::BTreeMap<String, Vec<StructFieldDecl>>,
    visiting: &mut std::collections::BTreeSet<String>,
    resolved: &mut std::collections::BTreeSet<String>,
    errors: &mut Vec<CodeParseError>,
) -> usize {
    match value_type {
        ValueType::Int => 1,
        ValueType::Void => 0,
        ValueType::Struct(idx) => {
            let name = scope
                .struct_definitions
                .get(*idx)
                .map(|d| d.name.clone())
                .unwrap_or_default();
            resolve_struct_definition(&name, scope, field_map, visiting, resolved, errors);
            scope
                .struct_definitions
                .get(*idx)
                .map(|d| d.total_size)
                .unwrap_or(0)
        }
        ValueType::Array(inner, size) => {
            let inner_size = value_type_size(inner, scope, field_map, visiting, resolved, errors);
            inner_size * size
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
