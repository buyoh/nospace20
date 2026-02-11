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

use scope::{FunctionIndex, Identifier, ScopeBuilder, ScopeResolver, ScopeType};

use crate::{
    base::CodeParseError,
    code_parse_error,
    tree_parser::{Expression, LocatedStatement, Operator1, Operator2, Statement},
};

pub use scope::{Function, Scope};
pub use types::{BuiltinFunctionKind, IdentifierRef};
pub(crate) use types::{Block, ExecExpression, ExecStatement, Variable};

/// 式を ExecExpression に変換する（識別子解決あり）
///
/// ScopeResolver を使用して変数名・関数名を IdentifierRef に解決する。
fn convert_to_exec_expression_with_resolver(
    expr: &Box<Expression>,
    parent_resolver: &ScopeResolver,
) -> Result<Box<ExecExpression>, Vec<CodeParseError>> {
    match expr.as_ref() {
        Expression::Operation1(Operator1::Ref, inner) => {
            // & は変数または配列要素に対してのみ使用可能
            match inner.as_ref() {
                Expression::Variable(name) => {
                    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                        vec![code_parse_error!(format!("undefined variable: {}", name))]
                    })?;
                    Ok(Box::new(ExecExpression::Operation1(
                        Operator1::Ref,
                        Box::new(ExecExpression::Variable(id_ref)),
                    )))
                }
                Expression::ArrayAccess(name, index_expr) => {
                    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                        vec![code_parse_error!(format!("undefined variable: {}", name))]
                    })?;

                    // 配列変数であることを確認
                    let array_size = parent_resolver
                        .get_array_size(name)
                        .ok_or_else(|| {
                            vec![code_parse_error!(format!("undefined variable: {}", name))]
                        })?
                        .ok_or_else(|| {
                            vec![code_parse_error!(format!("'{}' is not an array", name))]
                        })?;

                    let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;

                    Ok(Box::new(ExecExpression::Operation1(
                        Operator1::Ref,
                        Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)),
                    )))
                }
                _ => Err(vec![code_parse_error!(
                    "reference operator (&) can only be applied to variables or array elements"
                )]),
            }
        }
        Expression::Operation1(op, x) => Ok(Box::new(ExecExpression::Operation1(
            op.to_owned(),
            convert_to_exec_expression_with_resolver(&x, parent_resolver)?,
        ))),
        Expression::Operation2(op, l, r) => {
            // 複合代入演算子 (+=, -=, *=, /=, %=) を a = a + b の形式に展開
            let (actual_op, actual_l, actual_r) = match op {
                Operator2::PlusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Plus, l.clone(), r.clone())),
                ),
                Operator2::MinusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Minus, l.clone(), r.clone())),
                ),
                Operator2::MultiplyAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Multiply, l.clone(), r.clone())),
                ),
                Operator2::DivideAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Divide, l.clone(), r.clone())),
                ),
                Operator2::ModuloAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(Expression::Operation2(Operator2::Modulo, l.clone(), r.clone())),
                ),
                _ => (op.to_owned(), l, r),
            };
            
            Ok(Box::new(ExecExpression::Operation2(
                actual_op,
                convert_to_exec_expression_with_resolver(&actual_l, parent_resolver)?,
                convert_to_exec_expression_with_resolver(&actual_r, parent_resolver)?,
            )))
        }
        Expression::If(cond, stat1, stat2) => {
            let (s1, es1) = analyze_internal_with_parent(
                stat1,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
                // Phase 5: If/While の中ではglobal_functionsを使わない（関数宣言不可）
                // だだし、型の一貫性のために仮の可変参照を渡す
                &mut Vec::new(),
                &mut Vec::new(),
            )?;
            let (s2, es2) = analyze_internal_with_parent(
                stat2,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
                &mut Vec::new(),
                &mut Vec::new(),
            )?;
            Ok(Box::new(ExecExpression::If(
                convert_to_exec_expression_with_resolver(cond, parent_resolver)?,
                Block {
                    scope: s1.build(Vec::new(), Vec::new(), Vec::new()), // root_statementsは空
                    statements: es1,
                },
                Block {
                    scope: s2.build(Vec::new(), Vec::new(), Vec::new()), // root_statementsは空
                    statements: es2,
                },
            )))
        }
        Expression::While(expr, stat) => {
            let (s, es) = analyze_internal_with_parent(
                stat,
                ScopeType::Block,
                Vec::new(),
                Some(parent_resolver),
                &mut Vec::new(),
                &mut Vec::new(),
            )?;
            Ok(Box::new(ExecExpression::While(
                convert_to_exec_expression_with_resolver(expr, parent_resolver)?,
                Block {
                    scope: s.build(Vec::new(), Vec::new(), Vec::new()), // root_statementsは空
                    statements: es,
                },
            )))
        }
        Expression::Function(f, a) => {
            // Phase 5: 組み込み関数とユーザー定義関数を区別
            let mut args = Vec::new();
            for e in a {
                args.push(convert_to_exec_expression_with_resolver(
                    e,
                    parent_resolver,
                )?);
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
                _ => None,
            };

            if let Some(kind) = builtin_kind {
                // 組み込み関数
                Ok(Box::new(ExecExpression::BuiltinFunction(kind, args)))
            } else {
                // ユーザー定義関数：resolve する
                let func_ref = parent_resolver.resolve_function(f).ok_or_else(|| {
                    vec![code_parse_error!(format!("undefined function: {}", f))]
                })?;
                Ok(Box::new(ExecExpression::UserFunction(func_ref, args)))
            }
        }
        Expression::Factor(v) => Ok(Box::new(ExecExpression::Factor(v.to_owned()))),
        Expression::Variable(v) => {
            // 変数名を解決
            let var_ref = parent_resolver
                .resolve_variable(v)
                .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", v))])?;
            Ok(Box::new(ExecExpression::Variable(var_ref)))
        }
        Expression::ArrayAccess(name, index_expr) => {
            let id_ref = parent_resolver
                .resolve_variable(name)
                .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?;

            // 配列変数であることを確認
            let array_size = parent_resolver
                .get_array_size(name)
                .ok_or_else(|| vec![code_parse_error!(format!("undefined variable: {}", name))])?
                .ok_or_else(|| vec![code_parse_error!(format!("'{}' is not an array", name))])?;

            let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver)?;

            Ok(Box::new(ExecExpression::ArrayAccess(id_ref, exec_index, array_size)))
        }
        // パースエラー時のみ Invalid が生成されるため、正常系では到達しない
        Expression::Invalid(_) => {
            unreachable!("Expression::Invalid should not reach semantic analysis")
        }
    }
}

fn analyze_internal(
    statements: &Vec<LocatedStatement>,
    scope_type: ScopeType,
    global_functions: &mut Vec<Function>,
    global_function_names: &mut Vec<String>,
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    analyze_internal_with_parent(statements, scope_type, Vec::new(), None, global_functions, global_function_names)
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
) -> Result<(ScopeBuilder, Vec<ExecStatement>), Vec<CodeParseError>> {
    let mut scope = ScopeBuilder::new();

    // グローバル変数は暗黙的に static
    let is_static = matches!(scope_type, ScopeType::Root);
    let is_function_scope = matches!(scope_type, ScopeType::Root | ScopeType::Function);

    // 初期変数を登録（関数の引数など）
    for var_name in initial_vars {
        scope.add_variable(
            &var_name,
            Variable {
                slot_index: 0, // build() で正しい値に設定される
                is_static: false, // 関数引数は非 static
                array_size: None, // 関数引数は配列ではない
            },
        )?;
    }

    // 3パス解析
    // パス1a: 関数宣言を先にスキャンして登録（ホイスティング対応）
    // Phase 5: ネスト関数のサポート
    // Phase 5 修正: scope.functions ではなく global_functions に登録
    for located_stat in statements {
        let stat = &located_stat.statement;
        match stat {
            Statement::FunctionDeclaration(name, _, _) => {
                // 関数を仮登録（本体は後で解析）
                // とりあえず空の関数をプレースホルダーとして登録
                let global_idx = global_functions.len();
                global_function_names.push(name.clone());
                global_functions.push(Function {
                    arg_indices: Vec::new(),
                    block: Block {
                        scope: Scope {
                            identifier_map: BTreeMap::new(),
                            variable_indices: BTreeMap::new(),
                            variable_name_to_var_index: BTreeMap::new(),
                            variables: Vec::new(),
                            variable_count: 0,
                            functions: Vec::new(),
                            function_names: Vec::new(),                            main_function_index: None,                            static_init_statements: Vec::new(),
                            root_statements: Vec::new(),
                        },
                        statements: Vec::new(),
                    },
                });
                // identifier_map にはグローバルインデックスを登録
                scope.identifier_map.insert(
                    name.clone(),
                    Identifier::Function(FunctionIndex(global_idx)),
                );
            }
            _ => {}
        }
    }

    // パス1b: 変数宣言収集（ホイスティング対応）
    for located_stat in statements {
        let stat = &located_stat.statement;
        match stat {
            Statement::VariableDeclaration(name, _, is_static_explicit, array_size) => {
                // グローバル変数は暗黙的に static、明示的 static も考慮
                let final_is_static = *is_static_explicit || is_static;
                scope.add_variable(
                    name,
                    Variable {
                        slot_index: 0, // build() で正しい値に設定される
                        is_static: final_is_static,
                        array_size: array_size.map(|n| n as usize),
                    },
                )?;
            }
            Statement::FunctionDeclaration(_name, _, _) => {
                // パス1aで処理済み
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
        function_names: Vec::new(), // 未使用
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
            is_function_scope,
        );
        new_resolver
    } else {
        let mut new_resolver = ScopeResolver::new();
        new_resolver.enter_scope(
            &temporary_scope.variable_indices,
            &temporary_scope.variable_name_to_var_index,
            &temporary_scope.variables,
            &temporary_scope.identifier_map,
            is_function_scope,
        );
        new_resolver
    };

    // パス2: 文の変換（識別子解決を伴う）
    let mut exec_statements = Vec::<ExecStatement>::new();
    for located_stat in statements {
        let stat = &located_stat.statement;
        let loc = &located_stat.location;
        match stat {
            Statement::VariableDeclaration(_, init, is_static_explicit, _) => {
                // 初期化式を変換（変数宣言自体はパス1で完了）
                let exec = ExecStatement::Expression(
                    convert_to_exec_expression_with_resolver(init, &resolver)?,
                );
                // static 変数の初期化式は分離する
                // - ルートスコープ: static 変数の初期化は非 static より先に実行
                // - 関数スコープ: static 変数の初期化は main 前に1回だけ実行
                if *is_static_explicit {
                    scope.static_init_statements.push(exec);
                } else {
                    exec_statements.push(exec);
                }
            }
            Statement::FunctionDeclaration(name, args, block) => {
                // 関数本体を解析（親resolverを渡してグローバル変数を参照可能にする）
                // Phase 5: global_functions と global_function_names を渡す
                let (s, es) = analyze_internal_with_parent(
                    block,
                    ScopeType::Function,
                    args.clone(),
                    Some(&resolver),
                    global_functions,
                    global_function_names,
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

                // Phase 5: パス1aで登録済みの関数を更新
                // identifier_map にはグローバルインデックスが格納されている
                let global_idx = if let Some(Identifier::Function(info)) = scope.identifier_map.get(name) {
                    info.0
                } else {
                    panic!("internal error: function should be pre-registered in pass 1a");
                };

                global_functions[global_idx] = Function {
                    arg_indices,
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
                exec_statements.push(ExecStatement::Return(
                    convert_to_exec_expression_with_resolver(e, &resolver)?,
                ));
            }
            Statement::Expression(e) => {
                // ルートスコープでも式文を許可（グローバル変数の初期化式）
                exec_statements.push(ExecStatement::Expression(
                    convert_to_exec_expression_with_resolver(e, &resolver)?,
                ));
            }
            Statement::Continue => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: continue statement outside of function"
                    )]);
                }
                exec_statements.push(ExecStatement::Continue);
            }
            Statement::Break => {
                if let ScopeType::Root = scope_type {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "semantic error: break statement outside of function"
                    )]);
                }
                exec_statements.push(ExecStatement::Break);
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
    analyze_internal(root, ScopeType::Root, &mut global_functions, &mut global_function_names)
        .map(|(scope, root_stmts)| scope.build(root_stmts, global_functions, global_function_names))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
