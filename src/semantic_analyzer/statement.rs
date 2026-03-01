//! # 文の変換（ExecStatement 生成）
//!
//! `analyze_internal_with_parent` の Pass 2 で行われる文変換ロジック。
//! 各 `Statement` バリアントを `ExecStatement` に変換する。

use std::collections::BTreeMap;

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{LocatedStatement, Operator2, Statement},
};

use super::{
    context::AnalyzeContext,
    expression::convert_to_exec_expression_with_resolver,
    scope::{Identifier, ScopeBuilder, ScopeResolver, ScopeType},
    types::{infer_block_type, Block, ConditionMode, ExecStatement, LocatedExecStatement, ValueType},
};

/// Pass 2: 文リストを ExecStatement リストに変換する
///
/// `analyze_internal_with_parent` の Pass 2 処理をまとめた関数。
/// スコープの構築（Pass 0/1a/1b）は呼び出し元で完了している前提。
///
/// # Arguments
/// - `statements` - 変換対象の AST 文リスト
/// - `scope_type` - 現在のスコープ種別（Root / Function / Block）
/// - `scope` - 構築中のスコープ（static 初期化文付加のため）
/// - `resolver` - 識別子解決器
/// - `effective_func_return_types` - 型チェックに使用する関数戻り値型
/// - `ctx` - 解析コンテキスト（グローバル関数リスト等）
pub(super) fn convert_to_exec_statements(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    scope: &mut ScopeBuilder,
    resolver: &ScopeResolver,
    effective_func_return_types: &[ValueType],
    ctx: &mut AnalyzeContext,
) -> Result<Vec<LocatedExecStatement>, Vec<CodeParseError>> {
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
                    if let crate::tree_parser::Expression::Operation2(
                        Operator2::Assign,
                        lhs_expr,
                        rhs_expr,
                    ) = &init.expression
                    {
                        // 初期化代入: rhs のみ変換し、Assign を直接構築（final チェックなし）
                        let exec_rhs = convert_to_exec_expression_with_resolver(
                            rhs_expr,
                            resolver,
                            effective_func_return_types,
                        )?;
                        super::expression::require_int_type(&exec_rhs, effective_func_return_types)?;
                        let exec_lhs = convert_to_exec_expression_with_resolver(
                            lhs_expr,
                            resolver,
                            effective_func_return_types,
                        )?;
                        super::expression::make_located_exec(
                            super::types::ExecExpression::Operation2(
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
                            resolver,
                            effective_func_return_types,
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
                // パス1aで登録済みの関数のグローバルインデックスを取得
                let global_idx =
                    if let Some(Identifier::Function(info)) = scope.identifier_map.get(name) {
                        info.0
                    } else {
                        panic!("internal error: function should be pre-registered in pass 1a");
                    };

                // 関数本体を解析（親resolverを渡してグローバル変数を参照可能にする）
                // global_functions と global_function_names を渡す
                // func_global_index を渡すことで、ネストされた関数から
                // この関数の static 変数にアクセスする際に正しいオフセットを参照可能にする
                let mut func_ctx = AnalyzeContext::new_function(
                    ctx.global_functions,
                    ctx.global_function_names,
                    global_idx,
                );
                let (s, es) = super::analyze_internal_with_parent(
                    block,
                    ScopeType::Function,
                    args.clone(),
                    Some(resolver),
                    &mut func_ctx,
                )?;
                // 非ルートスコープの build() には空の functions/function_names を渡す
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

                // 関数の戻り値型はパス1aで決定済みの値を使用
                let func_return_type =
                    if let Some(Identifier::Function(info)) = scope.identifier_map.get(name) {
                        info.2
                    } else {
                        panic!("internal error: function return_type should be in pass 1a info");
                    };

                ctx.global_functions[global_idx] = super::scope::Function {
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
                            resolver,
                            effective_func_return_types,
                        )?;
                        // return: の式は Int でなければならない
                        super::expression::require_int_type(&exec_e, effective_func_return_types)?;
                        exec_statements.push(LocatedExecStatement {
                            statement: ExecStatement::Return(Some(exec_e)),
                            location: loc.clone(),
                        });
                    }
                    None => {
                        // void return: 関数が int 型（return: expr; がある）場合はエラー
                        if let Some(idx) = ctx.func_global_index {
                            if ctx.global_functions[idx].return_type != ValueType::Void {
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
                        resolver,
                        effective_func_return_types,
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
                    resolver,
                    effective_func_return_types,
                )?;
                // void 型の式は条件式に使用不可
                super::expression::require_int_type(&exec_cond, effective_func_return_types)?;
                let (s, es) = super::analyze_block_for_expression(
                    stat,
                    resolver,
                    effective_func_return_types,
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
                // NOTE: func_global_index=None を渡す（元の動作と一致）—
                // for-init ブロック内での return 型チェックをスキップするため
                let (init_sb, init_es) = {
                    let mut for_init_ctx = AnalyzeContext {
                        global_functions: &mut *ctx.global_functions,
                        global_function_names: &mut *ctx.global_function_names,
                        func_global_index: None,
                        inherited_func_return_types: ctx.inherited_func_return_types.clone(),
                    };
                    super::analyze_internal_with_parent(
                        init_stmts,
                        ScopeType::Block,
                        Vec::new(),
                        Some(resolver),
                        &mut for_init_ctx,
                    )?
                };
                let init_scope = init_sb.build(Vec::new(), Vec::new(), Vec::new());

                // Step 2: for スコープのリゾルバを構築
                // 現在のスコープに init スコープを重ねることで、
                // cond/step/body から init 変数を scope_depth=1 でアクセス可能にする
                let mut for_resolver = super::scope::ScopeResolver {
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
                let (cond_sb, cond_es) = super::analyze_block_for_expression(
                    cond_stmts,
                    &for_resolver,
                    effective_func_return_types,
                )?;
                let cond_scope = cond_sb.build(Vec::new(), Vec::new(), Vec::new());

                // 条件ブロックの型チェック: 最後の式が int 型でなければならない
                let temp_cond_block = Block {
                    scope: cond_scope,
                    statements: cond_es,
                };
                if infer_block_type(&temp_cond_block, effective_func_return_types)
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
                let (step_sb, step_es) = super::analyze_block_for_expression(
                    step_stmts,
                    &for_resolver,
                    effective_func_return_types,
                )?;

                // Step 5: body ブロックを解析
                let (body_sb, body_es) = super::analyze_block_for_expression(
                    body_stmts,
                    &for_resolver,
                    effective_func_return_types,
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

    Ok(exec_statements)
}
