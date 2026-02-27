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

use scope::{FunctionIndex, Identifier, ScopeBuilder, ScopeResolver, ScopeType, SymbolTable};

use crate::{
    base::{CodeParseError, SourceLocation},
    code_parse_error,
    tree_parser::{Expression, LocatedExpression, LocatedStatement, Operator1, Operator2, Statement},
};

pub use scope::{Function, Scope};
pub(crate) use types::{Block, ConditionMode, ExecExpression, ExecStatement, InternalBuiltinFunctionKind, LocatedExecExpression, LocatedExecStatement, Variable};
pub use types::{BuiltinFunctionKind, IdentifierRef, ValueType};

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
                        vec![code_parse_error!(loc.start, format!("undefined variable: {}", name))]
                    })?;
                    Ok(make_located_exec(ExecExpression::Operation1(
                        Operator1::Ref,
                        make_located_exec(ExecExpression::Variable(id_ref), &inner.location),
                    ), loc))
                }
                Expression::ArrayAccess(name, index_expr) => {
                    let id_ref = parent_resolver.resolve_variable(name).ok_or_else(|| {
                        vec![code_parse_error!(loc.start, format!("undefined variable: {}", name))]
                    })?;

                    // arr[i] は *(&arr + i) と同義。配列でなくてもインデックスアクセス可能。
                    let array_size = parent_resolver
                        .get_array_size(name)
                        .ok_or_else(|| {
                            vec![code_parse_error!(loc.start, format!("undefined variable: {}", name))]
                        })?
                        .unwrap_or(1);

                    let exec_index =
                        convert_to_exec_expression_with_resolver(index_expr, parent_resolver, func_return_types)?;

                    Ok(make_located_exec(ExecExpression::Operation1(
                        Operator1::Ref,
                        make_located_exec(ExecExpression::ArrayAccess(id_ref, exec_index, array_size), &inner.location),
                    ), loc))
                }
                _ => Err(vec![code_parse_error!(
                    loc.start,
                    "reference operator (&) can only be applied to variables or array elements"
                )]),
            }
        }
        Expression::Operation1(op, x) => {
            let exec_x = convert_to_exec_expression_with_resolver(&x, parent_resolver, func_return_types)?;
            // void 型の式は単項演算のオペランドに使用不可
            require_int_type(&exec_x, func_return_types)?;
            Ok(make_located_exec(ExecExpression::Operation1(op.to_owned(), exec_x), loc))
        }
        Expression::Operation2(op, l, r) => {
            // 複合代入演算子 (+=, -=, *=, /=, %=) を a = a + b の形式に展開
            let (actual_op, actual_l, actual_r) = match op {
                Operator2::PlusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(
                            Operator2::Plus,
                            l.clone(),
                            r.clone(),
                        ),
                        location: loc.clone(),
                    }),
                ),
                Operator2::MinusAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(
                            Operator2::Minus,
                            l.clone(),
                            r.clone(),
                        ),
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
                        expression: Expression::Operation2(
                            Operator2::Divide,
                            l.clone(),
                            r.clone(),
                        ),
                        location: loc.clone(),
                    }),
                ),
                Operator2::ModuloAssign => (
                    Operator2::Assign,
                    l,
                    &Box::new(LocatedExpression {
                        expression: Expression::Operation2(
                            Operator2::Modulo,
                            l.clone(),
                            r.clone(),
                        ),
                        location: loc.clone(),
                    }),
                ),
                _ => (op.to_owned(), l, r),
            };

            let exec_l = convert_to_exec_expression_with_resolver(&actual_l, parent_resolver, func_return_types)?;
            let exec_r = convert_to_exec_expression_with_resolver(&actual_r, parent_resolver, func_return_types)?;

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

            Ok(make_located_exec(ExecExpression::Operation2(
                actual_op,
                exec_l,
                exec_r,
            ), loc))
        }
        Expression::If(cond, stat1, stat2) => {
            let exec_cond = convert_to_exec_expression_with_resolver(cond, parent_resolver, func_return_types)?;
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
            Ok(make_located_exec(ExecExpression::If(
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
            ), loc))
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
            Ok(make_located_exec(ExecExpression::Block(Block {
                scope: s.build(Vec::new(), Vec::new(), Vec::new()), // root_statementsは空
                statements: es,
            }), loc))
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
                    return Err(vec![code_parse_error!(loc.start, format!(
                        "builtin function '{}' expects {} argument(s), but {} were provided",
                        f,
                        expected,
                        args.len()
                    ))]);
                }
                // 組み込み関数
                Ok(make_located_exec(ExecExpression::BuiltinFunction(kind, args), loc))
            } else {
                // ユーザー定義関数：resolve する
                let func_ref = parent_resolver
                    .resolve_function(f)
                    .ok_or_else(|| vec![code_parse_error!(loc.start, format!("undefined function: {}", f))])?;

                // 引数数チェック
                let expected_count = parent_resolver
                    .get_function_arg_count(f)
                    .expect("function should be resolvable");
                if args.len() != expected_count {
                    return Err(vec![code_parse_error!(loc.start, format!(
                        "function '{}' expects {} argument(s), but {} were provided",
                        f,
                        expected_count,
                        args.len()
                    ))]);
                }

                Ok(make_located_exec(ExecExpression::UserFunction(func_ref, args), loc))
            }
        }
        Expression::Factor(v) => Ok(make_located_exec(ExecExpression::Factor(v.to_owned()), loc)),
        Expression::Variable(v) => {
            // 変数名を解決
            let var_ref = parent_resolver
                .resolve_variable(v)
                .ok_or_else(|| vec![code_parse_error!(loc.start, format!("undefined variable: {}", v))])?;
            Ok(make_located_exec(ExecExpression::Variable(var_ref), loc))
        }
        Expression::ArrayAccess(name, index_expr) => {
            let id_ref = parent_resolver
                .resolve_variable(name)
                .ok_or_else(|| vec![code_parse_error!(loc.start, format!("undefined variable: {}", name))])?;

            // arr[i] は *(&arr + i) と同義。配列でなくてもインデックスアクセス可能。
            let array_size = parent_resolver
                .get_array_size(name)
                .ok_or_else(|| vec![code_parse_error!(loc.start, format!("undefined variable: {}", name))])?
                .unwrap_or(1);

            let exec_index = convert_to_exec_expression_with_resolver(index_expr, parent_resolver, func_return_types)?;
            // 配列インデックスに void 型は使用不可
            require_int_type(&exec_index, func_return_types)?;

            Ok(make_located_exec(ExecExpression::ArrayAccess(
                id_ref, exec_index, array_size,
            ), loc))
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
                    is_dummy: false,
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
            Statement::VariableDeclaration(_, init, is_static_explicit, _) => {
                // 初期化式を変換（変数宣言自体はパス1で完了）
                let exec_stmt = ExecStatement::Expression(convert_to_exec_expression_with_resolver(
                    init, &resolver, &effective_func_return_types,
                )?);
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
                let func_return_type = if let Some(Identifier::Function(info)) = scope.identifier_map.get(name) {
                    info.2
                } else {
                    panic!("internal error: function return_type should be in pass 1a info");
                };

                global_functions[global_idx] = Function {
                    arg_indices,
                    return_type: func_return_type,
                    is_dummy: false,
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
                        let exec_e = convert_to_exec_expression_with_resolver(expr, &resolver, &effective_func_return_types)?;
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
                    statement: ExecStatement::Expression(
                        convert_to_exec_expression_with_resolver(e, &resolver, &effective_func_return_types)?,
                    ),
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
                let exec_cond = convert_to_exec_expression_with_resolver(expr, &resolver, &effective_func_return_types)?;
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
