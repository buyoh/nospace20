//! # 式の変換（ExecExpression 生成）
//!
//! AST の `Expression` を実行可能な `ExecExpression` に変換するロジック。
//! 変数/関数の識別子解決、型チェックを含む。

use crate::{
    base::{CodeParseError, SourceLocation},
    code_parse_error,
    tree_parser::{Expression, LocatedExpression, Operator1, Operator2, TypeSpec},
};

use super::{
    scope::ScopeResolver,
    types::{
        Block, BuiltinFunctionKind, ConditionMode, ExecExpression, LocatedExecExpression, ValueType,
    },
};

/// void 型の式が値として使われている場合にエラーを返す
pub(super) fn require_int_type(
    expr: &LocatedExecExpression,
    func_return_types: &[ValueType],
) -> Result<(), Vec<CodeParseError>> {
    let inferred = expr.infer_type(func_return_types);
    if inferred != ValueType::Int {
        let message = if inferred == ValueType::Void {
            "semantic error: cannot use void expression as a value"
        } else {
            "semantic error: cannot use non-int expression as a value"
        };
        Err(vec![code_parse_error!(message)])
    } else {
        Ok(())
    }
}

/// LocatedExecExpression を構築するヘルパー
pub(super) fn make_located_exec(
    expr: ExecExpression,
    location: &SourceLocation,
) -> Box<LocatedExecExpression> {
    Box::new(LocatedExecExpression {
        expression: expr,
        location: location.clone(),
    })
}

pub(super) fn resolve_type_spec(
    type_spec: &TypeSpec,
    resolver: &ScopeResolver,
) -> Result<ValueType, Vec<CodeParseError>> {
    match type_spec {
        TypeSpec::Int => Ok(ValueType::Int),
        TypeSpec::Void => Ok(ValueType::Void),
        TypeSpec::Named(name) => resolver
            .resolve_struct_index(name)
            .map(ValueType::Struct)
            .ok_or_else(|| {
                vec![code_parse_error!(format!(
                    "semantic error: undefined struct type '{}'",
                    name
                ))]
            }),
        TypeSpec::Array(inner, size) => {
            let inner_value = resolve_type_spec(inner, resolver)?;
            Ok(ValueType::Array(Box::new(inner_value), *size))
        }
        TypeSpec::Ref(_) => Ok(ValueType::Int),
    }
}

/// 式を ExecExpression に変換する（識別子解決あり）
///
/// ScopeResolver を使用して変数名・関数名を IdentifierRef に解決する。
/// func_return_types を使用して式の型チェックを行う。
pub(super) fn convert_to_exec_expression_with_resolver(
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
                    let (id_ref, value_type) = parent_resolver
                        .resolve_variable_with_type(name)
                        .ok_or_else(|| {
                            vec![code_parse_error!(
                                loc.start,
                                format!("undefined variable: {}", name)
                            )]
                        })?;
                    Ok(make_located_exec(
                        ExecExpression::Operation1(
                            Operator1::Ref,
                            make_located_exec(
                                ExecExpression::Variable(id_ref, value_type),
                                &inner.location,
                            ),
                        ),
                        loc,
                    ))
                }
                Expression::ArrayAccess(name, index_expr) => {
                    let (id_ref, _value_type) = parent_resolver
                        .resolve_variable_with_type(name)
                        .ok_or_else(|| {
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
                        let resolved_name = parent_resolver
                            .resolve_alias_chain(name)
                            .map_err(|e| vec![code_parse_error!(loc.start, e)])?;
                        if parent_resolver.is_final_variable(&resolved_name) {
                            return Err(vec![code_parse_error!(
                                loc.start,
                                format!("cannot assign to final variable '{}'", name)
                            )]);
                        }
                    }
                    Expression::ArrayAccess(name, _) => {
                        let resolved_name = parent_resolver
                            .resolve_alias_chain(name)
                            .map_err(|e| vec![code_parse_error!(loc.start, e)])?;
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
            let (s1, es1) =
                super::analyze_block_for_expression(stat1, parent_resolver, func_return_types)?;
            let (s2, es2) =
                super::analyze_block_for_expression(stat2, parent_resolver, func_return_types)?;
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
        Expression::TypeAnnotation(inner_expr, type_spec) => {
            let expected_type = resolve_type_spec(type_spec, parent_resolver)?;
            let actual_expr = convert_to_exec_expression_with_resolver(
                inner_expr,
                parent_resolver,
                func_return_types,
            )?;
            let actual_type = actual_expr.infer_type(func_return_types);

            if expected_type == ValueType::Void && actual_type == ValueType::Int {
                return Ok(make_located_exec(ExecExpression::VoidCast(actual_expr), loc));
            }

            if let ValueType::Struct(expected_idx) = expected_type.clone() {
                if let Some(struct_def) = parent_resolver.get_struct_definition(expected_idx) {
                    match (&inner_expr.expression, actual_type) {
                        (Expression::Variable(name), ValueType::Array(_, size)) => {
                            if size < struct_def.total_size {
                                return Err(vec![code_parse_error!(
                                    loc.start,
                                    format!(
                                        "type mismatch: array size {} is smaller than struct '{}' size {}",
                                        size, struct_def.name, struct_def.total_size
                                    )
                                )]);
                            }
                            let (id_ref, value_type) = parent_resolver
                                .resolve_variable_with_type(name)
                                .ok_or_else(|| {
                                    vec![code_parse_error!(
                                        loc.start,
                                        format!("undefined variable: {}", name)
                                    )]
                                })?;
                            let addr_expr = make_located_exec(
                                ExecExpression::Operation1(
                                    Operator1::Ref,
                                    make_located_exec(
                                        ExecExpression::Variable(id_ref, value_type),
                                        &inner_expr.location,
                                    ),
                                ),
                                &inner_expr.location,
                            );
                            return Ok(make_located_exec(
                                ExecExpression::TypeAssertion(addr_expr, expected_type),
                                loc,
                            ));
                        }
                        (_, ValueType::Struct(actual_idx)) if actual_idx == expected_idx => {
                            return Ok(make_located_exec(
                                ExecExpression::TypeAssertion(actual_expr, expected_type),
                                loc,
                            ));
                        }
                        _ => {
                            return Err(vec![code_parse_error!(loc.start, "type mismatch")]);
                        }
                    }
                }
            }

            if expected_type != actual_type {
                return Err(vec![code_parse_error!(loc.start, "type mismatch")]);
            }

            Ok(make_located_exec(
                ExecExpression::TypeAssertion(actual_expr, expected_type),
                loc,
            ))
        }
        Expression::FieldAccess(base_expr, field_name) => {
            let exec_base = convert_to_exec_expression_with_resolver(
                base_expr,
                parent_resolver,
                func_return_types,
            )?;
            let base_type = exec_base.infer_type(func_return_types);
            let struct_idx = match base_type {
                ValueType::Struct(idx) => idx,
                _ => {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "field access on non-struct value"
                    )]);
                }
            };
            let struct_def = parent_resolver.get_struct_definition(struct_idx).ok_or_else(|| {
                vec![code_parse_error!(loc.start, "unknown struct type")]
            })?;
            let field = struct_def
                .fields
                .iter()
                .find(|f| f.name == *field_name)
                .ok_or_else(|| {
                    vec![code_parse_error!(
                        loc.start,
                        format!("undefined field '{}'", field_name)
                    )]
                })?;
            let field_type = field.value_type.clone();
            let array_size = match &field_type {
                ValueType::Array(_, size) => Some(*size),
                _ => None,
            };
            Ok(make_located_exec(
                ExecExpression::StructFieldAccess(
                    exec_base,
                    field.offset,
                    array_size,
                    field_type,
                ),
                loc,
            ))
        }
        Expression::FieldArrayAccess(base_expr, field_name, index_expr) => {
            let exec_base = convert_to_exec_expression_with_resolver(
                base_expr,
                parent_resolver,
                func_return_types,
            )?;
            let base_type = exec_base.infer_type(func_return_types);
            let struct_idx = match base_type {
                ValueType::Struct(idx) => idx,
                _ => {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        "field access on non-struct value"
                    )]);
                }
            };
            let struct_def = parent_resolver.get_struct_definition(struct_idx).ok_or_else(|| {
                vec![code_parse_error!(loc.start, "unknown struct type")]
            })?;
            let field = struct_def
                .fields
                .iter()
                .find(|f| f.name == *field_name)
                .ok_or_else(|| {
                    vec![code_parse_error!(
                        loc.start,
                        format!("undefined field '{}'", field_name)
                    )]
                })?;
            let array_size = match &field.value_type {
                ValueType::Array(_, size) => *size,
                _ => {
                    return Err(vec![code_parse_error!(
                        loc.start,
                        format!("field '{}' is not an array", field_name)
                    )]);
                }
            };
            let exec_index = convert_to_exec_expression_with_resolver(
                index_expr,
                parent_resolver,
                func_return_types,
            )?;
            require_int_type(&exec_index, func_return_types)?;
            Ok(make_located_exec(
                ExecExpression::StructFieldArrayAccess(
                    exec_base,
                    field.offset,
                    exec_index,
                    array_size,
                ),
                loc,
            ))
        }
        Expression::StructLiteral(_, _) => Err(vec![code_parse_error!(
            loc.start,
            "semantic error: struct literal can only be used in struct initialization"
        )]),
        Expression::Block(statements) => {
            let (s, es) = super::analyze_block_for_expression(
                statements,
                parent_resolver,
                func_return_types,
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
            // 組み込み関数とユーザー定義関数を区別
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

            // まず alias を解決（組み込み関数へのエイリアスもサポートするため）
            let resolved_f = parent_resolver
                .resolve_alias_chain(f)
                .map_err(|e| vec![code_parse_error!(loc.start, e)])?;

            // 組み込み関数のリスト（__ で始まる）
            // alias 解決後の名前で BuiltinFunctionKind に変換
            let builtin_kind = BuiltinFunctionKind::convert_name_to_builtin(&resolved_f);

            if let Some(kind) = builtin_kind {
                // 組み込み関数の引数数チェック
                let expected = kind.args_count();
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
                // ブロックエイリアスのチェック: alias チェーン解決後の名前で検索
                if let Some(block_body) = parent_resolver.resolve_block_alias(&resolved_f) {
                    // ブロックエイリアスに引数は不可
                    if !args.is_empty() {
                        return Err(vec![code_parse_error!(
                            loc.start,
                            format!("block alias '{}' cannot be called with arguments", f)
                        )]);
                    }
                    // ブロックエイリアスをインライン展開: 呼び出し元スコープで本体を解析
                    let block_body_clone = block_body.clone();
                    let (s, es) = super::analyze_block_for_expression(
                        &block_body_clone,
                        parent_resolver,
                        func_return_types,
                    )?;
                    return Ok(make_located_exec(
                        ExecExpression::Block(Block {
                            scope: s.build(Vec::new(), Vec::new(), Vec::new()),
                            statements: es,
                        }),
                        loc,
                    ));
                }

                let func_ref = parent_resolver
                    .resolve_function(&resolved_f)
                    .ok_or_else(|| {
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
            let resolved_name = parent_resolver
                .resolve_alias_chain(v)
                .map_err(|e| vec![code_parse_error!(loc.start, e)])?;

            // まず constexpr テーブルを確認（定数式への置換）
            if let Some(const_val) = parent_resolver.resolve_constexpr(&resolved_name) {
                return Ok(make_located_exec(ExecExpression::Factor(const_val), loc));
            }
            // 変数名を解決
            let (var_ref, value_type) = parent_resolver
                .resolve_variable_with_type(&resolved_name)
                .ok_or_else(|| {
                    vec![code_parse_error!(
                        loc.start,
                        format!("undefined variable: {}", v)
                    )]
                })?;
            Ok(make_located_exec(
                ExecExpression::Variable(var_ref, value_type),
                loc,
            ))
        }
        Expression::ArrayAccess(name, index_expr) => {
            let (id_ref, _value_type) = parent_resolver
                .resolve_variable_with_type(name)
                .ok_or_else(|| {
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
