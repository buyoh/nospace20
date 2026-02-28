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

use std::collections::BTreeMap;

use alias::{collect_alias_map, collect_block_alias_map, detect_block_alias_cycles};
use constexpr::collect_constexpr_table;
use return_analysis::{guarantees_return, has_return_statement};
use scope::{FunctionIndex, Identifier, ScopeBuilder, ScopeResolver, ScopeType, SymbolTable};
use template::expand_template_instantiations;

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{LocatedStatement, Statement},
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
    initial_vars: Vec<String>,
    parent_resolver: Option<&ScopeResolver>,
    ctx: &mut context::AnalyzeContext,
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
    for located_stat in statements {
        let stat = &located_stat.statement;
        match stat {
            Statement::FunctionDeclaration(name, args, body) => {
                let global_idx = ctx.global_functions.len();

                let has_ret = has_return_statement(body);
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

                ctx.global_function_names.push(name.clone());
                ctx.global_functions.push(Function {
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
    let effective_func_return_types: Vec<ValueType> = if ctx.inherited_func_return_types.is_empty() {
        ctx.global_functions.iter().map(|f| f.return_type).collect()
    } else {
        ctx.inherited_func_return_types.clone()
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

    // 親のリゾルバを継承して新しいリゾルバを作成
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
            ctx.func_global_index,
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
    let mut ctx = context::AnalyzeContext::new_root(
        &mut global_functions,
        &mut global_function_names,
    );
    analyze_internal(root, ScopeType::Root, &mut ctx)
        .map(|(scope, root_stmts)| scope.build(root_stmts, global_functions, global_function_names))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
