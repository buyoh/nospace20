//! # Semantic Analyzer
//!
//! 意味解析器。ASTを実行可能な構造に変換する。
//!
//! ## モジュール構成
//!
//! - `exec_context` : 実行文脈（ExecContext。ルートの初期化・関数本体・static の初期化子とループの深さ）
//! - `function_table` : 意味解析中のグローバル関数表（FunctionTable 構造体）
//! - `program`    : 意味解析の結果（Program 構造体。ルートスコープとプログラム全体の表）
//! - `expression` : 式の変換処理（ExecExpression 生成）
//! - `statement`  : 文の変換処理（ExecStatement 生成）
//! - `scope`      : スコープ・識別子解決
//! - `types`      : 中間表現の型定義
//! - `alias`      : エイリアス収集・解決
//! - `analysis_tables` : 全スコープで共有する表（関数表・グローバル領域の割り当て・static の初期化文）
//! - `constexpr`  : コンパイル時定数
//! - `template`   : テンプレート展開
//! - `return_analysis` : return 文制御フロー解析

mod alias;
mod analysis_tables;
mod constexpr;
mod exec_context;
mod expression;
mod function_table;
mod place;
mod program;
mod return_analysis;
mod scope;
mod statement;
mod template;
mod types;

use std::collections::{BTreeMap, BTreeSet};

use alias::{collect_alias_map, collect_block_alias_map, detect_block_alias_cycles};
use analysis_tables::AnalysisTables;
use constexpr::collect_constexpr_table;
use return_analysis::{guarantees_return, has_return_statement};
use scope::{FunctionIndex, Identifier, ScopeBuilder, ScopeResolver, ScopeType};
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

pub use program::Program;
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
/// 親の関数表を共有するため、ブロック内で宣言した関数も全体の関数表に登録される。
///
/// バグ: 以前は一時的な空の関数表を渡していたため、ブロック内の関数のインデックスが 0 から振られ、
/// 解析後にその表が捨てられた。呼び出しは本来の関数表の別の関数（多くは __main）を指していた。
/// 解決策: 親の関数表（`FunctionTable`）を共有し、ブロック内の関数も全体の関数表に登録する。
fn analyze_block_for_expression(
    statements: &Vec<LocatedStatement>,
    parent_resolver: &ScopeResolver,
    tables: &mut AnalysisTables,
) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>> {
    analyze_internal_with_parent(
        statements,
        ScopeType::Block,
        None,
        Some(parent_resolver),
        tables,
    )
}

fn analyze_internal(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    tables: &mut AnalysisTables,
) -> Result<(ScopeBuilder, Vec<LocatedExecStatement>), Vec<CodeParseError>> {
    analyze_internal_with_parent(statements, scope_type, None, None, tables)
}

/// 関数本体のスコープに事前登録する変数（関数の引数）
struct InitialVars {
    /// 変数名と型注釈（宣言順）
    vars: Vec<(String, Option<TypeSpec>)>,
    /// 引数を宣言した関数宣言の文の開始位置（引数ごとの位置は AST に無いので、エラー位置に使う）
    declaration_location: usize,
}

/// 初期変数と親のリゾルバを指定して解析する
///
/// - `statements`      : 解析対象の文リスト
/// - `scope_type`      : スコープ種別（Root / Function / Block）
/// - `initial_vars`    : 事前登録する変数（関数の引数。関数本体以外は None）
/// - `parent_resolver` : 親スコープのリゾルバ（None = ルートスコープ）
/// - `tables`          : 全スコープで共有する表
fn analyze_internal_with_parent(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    initial_vars: Option<InitialVars>,
    parent_resolver: Option<&ScopeResolver>,
    tables: &mut AnalysisTables,
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
    let is_function_scope = matches!(scope_type, ScopeType::Root | ScopeType::Function { .. });

    // 初期変数を登録（関数の引数）
    if let Some(InitialVars {
        vars,
        declaration_location,
    }) = initial_vars
    {
        for (var_name, type_annot) in vars {
            let value_type = match type_annot {
                None => ValueType::Int,
                Some(spec) => match spec {
                    TypeSpec::Int => ValueType::Int,
                    _ => {
                        return Err(vec![code_parse_error!(
                            declaration_location,
                            format!("semantic error: argument '{}' must be int type", var_name)
                        )]);
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
                    global_slot: None,
                },
                declaration_location,
            )?;
        }
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
    // バグ: 関数宣言のスキャンは "" から、パス 2 の参照は親から継承したプレフィックスから名前を作っていたため、
    // 名前空間内の関数の本体（とその中のブロック）で宣言した関数が見つからず panic していた。
    // 解決策: スキャンも親の名前空間プレフィックスから始め、パス 2 と同じ名前で登録する。
    let ns_prefix = parent_resolver
        .map(|p| p.current_ns_prefix())
        .unwrap_or_default();
    scan_function_declarations(statements, &ns_prefix, &mut scope, tables)?;

    // パス1c: 変数宣言収集（ホイスティング対応）
    // 名前空間内の変数もマングル名で登録する
    scan_variable_declarations(
        statements,
        "",
        is_static,
        &mut scope,
        &mut tables.global_slots,
    )?;

    // 変数名からインデックスへのマッピングを先に構築（resolver で使用）
    // 配列サイズを考慮したスロットインデックスを使用
    // ここで合計が usize に収まることを検査する（ScopeBuilder::build() は同じ計算を検査済みとして扱う）
    let slot_layout = scope.compute_slot_layout().map_err(|var_name| {
        vec![code_parse_error!(format!(
            "semantic error: total size of variables in a scope is too large (at '{}')",
            var_name
        ))]
    })?;
    // ルートの変数の直後から、関数スコープ・ブロックスコープの static 変数を割り当てる
    // （関数・ブロックはルートのパス 2 で解析されるので、必ずこの後に割り当てられる）
    if matches!(scope_type, ScopeType::Root) {
        tables.global_slots.start(slot_layout.variable_count);
    }

    // Variable を Clone するための一時保存（resolver が参照するため）
    // scope.variables をそのまま使用するのではなく、Scope にまとめて後で参照
    // 一旦 temporary_scope を作って参照を保持
    // identifier_map も保持して関数解決に使用
    let temporary_scope = Scope {
        identifier_map: scope.identifier_map.clone(), // 関数解決に必要
        variable_indices: slot_layout.variable_indices,
        variable_name_to_var_index: slot_layout.variable_name_to_var_index,
        variables: scope.variables.clone(), // Clone が必要
        variable_count: slot_layout.variable_count,
        struct_definitions: scope.struct_definitions.clone(),
        struct_name_to_index: scope.struct_name_to_index.clone(),
    };

    // 親のリゾルバを継承して新しいリゾルバを作成
    let mut resolver = if let Some(parent) = parent_resolver {
        // 関数本体は新しい実行単位。ブロックは親の実行文脈を引き継ぐ
        let exec = match scope_type {
            ScopeType::Function { func_index } => {
                exec_context::ExecContext::function_body(func_index, parent.scope_stack.len())
            }
            ScopeType::Root | ScopeType::Block => parent.exec,
        };
        let mut new_resolver = ScopeResolver {
            scope_stack: parent.scope_stack.clone(),
            namespace_prefix: parent.namespace_prefix.clone(),
            import_table: import_bundle.import_table.clone(),
            exec,
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
        );
        new_resolver
    };

    // パス2: 文の変換（statement モジュールに委譲）
    let exec_statements = statement::convert_to_exec_statements(
        statements, scope_type, &mut scope, &resolver, tables,
    )?;

    resolver.leave_scope();
    Ok((scope, exec_statements))
}

pub fn analyze(root: &Vec<LocatedStatement>) -> Result<Program, Vec<CodeParseError>> {
    let mut tables = AnalysisTables::new();
    let (scope, root_stmts) = analyze_internal(root, ScopeType::Root, &mut tables)?;
    let AnalysisTables {
        functions,
        global_slots,
        static_init_statements,
    } = tables;
    let (global_functions, global_function_names) = functions.into_parts();
    let root_scope = scope.build();
    // グローバル領域 = ルートの変数 + 関数スコープ・ブロックスコープのすべての static 変数
    debug_assert!(global_slots.total() >= root_scope.variable_count);
    Ok(Program::new(
        root_scope,
        global_functions,
        global_function_names,
        static_init_statements,
        root_stmts,
        global_slots.total(),
    ))
}

/// 名前空間の解析中に集めた `import` 宣言 1 件
#[derive(Clone)]
struct ImportDecl {
    /// 宣言がある名前空間（区切りは `$`、ルートは空文字列）
    current_ns: String,
    /// 宣言に書かれた名前空間名（未解決）
    target_ns_name: String,
    is_weak: bool,
    is_export: bool,
    /// `import` 文の開始文字インデックス（エラー位置に使う）
    location: usize,
}

/// import 解決の結果
struct ImportBundle {
    /// 名前空間 → import された名前 → 完全修飾名（`ScopeResolver.import_table` と同じ形）
    import_table: BTreeMap<String, BTreeMap<String, String>>,
    /// export された完全修飾名 → import 元の完全修飾名
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
    tables: &mut AnalysisTables,
) -> Result<(), Vec<CodeParseError>> {
    for located_stat in statements {
        match &located_stat.statement {
            Statement::FunctionDeclaration(name, args, body, return_type_annot) => {
                let mangled_name = format!("{}{}", ns_prefix, name);

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

                let global_idx = tables.functions.declare(mangled_name.clone(), return_type);
                // identifier_map にはグローバルインデックスと引数数を登録
                scope.add_identifier(
                    &mangled_name,
                    Identifier::Function(FunctionIndex(global_idx, args.len())),
                    located_stat.location.start,
                )?;
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                scan_function_declarations(body, &sub_prefix, scope, tables)?;
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
    global_slots: &mut analysis_tables::GlobalSlotAllocator,
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
                // バグ: `n as usize` は usize が 32bit の環境（wasm32）で上位ビットを切り捨てていた。
                // 解決策: usize::try_from で変換し、収まらない場合は意味解析エラーにする。
                let mut effective_array_size = match array_size {
                    Some(n) => Some(usize::try_from(*n).map_err(|_| {
                        vec![code_parse_error!(
                            located_stat.location.start,
                            format!(
                                "semantic error: array size of '{}' is too large",
                                mangled_name
                            )
                        )]
                    })?),
                    None => None,
                };
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
                        global_slot: None,
                    },
                    located_stat.location.start,
                )?;

                // 関数スコープ・ブロックスコープの static 変数は、グローバル領域に置く
                // バグ: ブロックの static はブロックのフレームのスロットに置かれ、呼び出しをまたいで値が残らず、
                // 初期化文も実行されなかった。関数の static も関数ごとの領域を各バックエンドが推測して引いていた。
                // 解決策: ルートの変数の後ろのグローバル領域にスロットを割り当て、is_global の参照で引く
                // （add_variable の成功後に割り当てるので、重複宣言のエラーでスロットだけが進むことは無い）
                if !is_static && *is_static_explicit {
                    let slot = global_slots
                        .allocate(effective_array_size.unwrap_or(1))
                        .ok_or_else(|| {
                            vec![code_parse_error!(
                                located_stat.location.start,
                                format!(
                                    "semantic error: total size of global and static variables is too large (at '{}')",
                                    mangled_name
                                )
                            )]
                        })?;
                    let var = scope
                        .variables
                        .last_mut()
                        .expect("the variable was just added");
                    var.global_slot = Some(slot);
                }
            }
            Statement::NamespaceDeclaration(ns_name, body) => {
                let sub_prefix = format!("{}{}$", ns_prefix, ns_name);
                scan_variable_declarations(body, &sub_prefix, is_static, scope, global_slots)?;
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

        let size = value_type_size(
            &value_type,
            name,
            scope,
            field_map,
            visiting,
            resolved,
            errors,
        );
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
        // バグ: 未チェックの `+=` で、巨大なフィールドが並ぶと debug では panic、release では wrap していた。
        // 解決策: checked_add にし、usize に収まらない場合は意味解析エラーにする。
        offset = match offset.checked_add(size) {
            Some(next) => next,
            None => {
                errors.push(code_parse_error!(format!(
                    "semantic error: size of struct '{}' is too large",
                    name
                )));
                visiting.remove(name);
                return;
            }
        };
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

/// 値の型のサイズ（スロット数）を返す。失敗時（void・未解決の struct・サイズのオーバーフロー）は 0
///
/// `struct_name` はエラーメッセージに出す、フィールドを持つ struct の名前。
fn value_type_size(
    value_type: &ValueType,
    struct_name: &str,
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
            let inner_size = value_type_size(
                inner,
                struct_name,
                scope,
                field_map,
                visiting,
                resolved,
                errors,
            );
            // バグ: 未チェックの `*` で、debug では panic、release では wrap していた。
            // 解決策: checked_mul にし、usize に収まらない場合はエラーを積んで失敗（0）を返す。
            inner_size.checked_mul(*size).unwrap_or_else(|| {
                errors.push(code_parse_error!(format!(
                    "semantic error: size of a field in '{}' is too large",
                    struct_name
                )));
                0
            })
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
