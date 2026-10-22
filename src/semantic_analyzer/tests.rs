//! Semantic Analyzer のテストコード

use super::*;
use crate::base::SourceLocation;
use crate::tree_parser::LocatedExpression;

/// Expression を LocatedExpression に包むヘルパー関数（テスト位置情報はダミー）
fn loc_expr(expression: Expression) -> Box<LocatedExpression> {
    Box::new(LocatedExpression {
        expression,
        location: SourceLocation::new(0, 0),
    })
}

#[test]
fn test_error_return_outside_function() {
    // return:0; at root level should error with position
    let statements = vec![LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(10, 20),
    }];

    let result = analyze(&statements);
    assert!(result.is_err());

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code_pointer, Some(10));
    assert!(errors[0]
        .message
        .contains("return statement outside of function"));
}

#[test]
fn test_error_break_outside_function() {
    let statements = vec![LocatedStatement {
        statement: Statement::Break,
        location: SourceLocation::new(25, 30),
    }];

    let result = analyze(&statements);
    assert!(result.is_err());

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code_pointer, Some(25));
    assert!(errors[0]
        .message
        .contains("break statement outside of function"));
}

#[test]
fn test_error_continue_outside_function() {
    let statements = vec![LocatedStatement {
        statement: Statement::Continue,
        location: SourceLocation::new(35, 45),
    }];

    let result = analyze(&statements);
    assert!(result.is_err());

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code_pointer, Some(35));
    assert!(errors[0]
        .message
        .contains("continue statement outside of function"));
}

#[test]
fn test_success_expression_at_root_level() {
    // グローバル変数の初期化式を許可
    let statements = vec![LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::Factor(42))),
        location: SourceLocation::new(50, 55),
    }];

    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_success_block_scoped_variable() {
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            loc_expr(Expression::Factor(0)),
            false, // non-static
            false,
            None, // not an array
            None,
        ),
        location: SourceLocation::new(150, 160),
    };

    // ブロックスコープでの変数宣言をシミュレート
    // If式のthen節内で変数宣言を試みる
    let if_expr = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::If(
            loc_expr(Expression::Factor(1)),
            vec![var_decl], // block内の変数宣言
            vec![],
        ))),
        location: SourceLocation::new(140, 170),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration("test".to_string(), vec![], vec![if_expr], None),
        location: SourceLocation::new(135, 175),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_success_global_variable() {
    // グローバル変数を許可
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "global".to_string(),
            loc_expr(Expression::Factor(42)),
            false, // non-static explicitly, but global is implicitly static
            false,
            None, // not an array
            None,
        ),
        location: SourceLocation::new(200, 210),
    };

    let statements = vec![var_decl];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    // グローバル変数が登録されていることを確認
    assert_eq!(scope.variable_count, 1);
    assert!(scope.variables[0].is_static); // 暗黙的に static
}

#[test]
fn test_success_simple_function() {
    // func: main() { return:0; }
    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(20, 30),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 35),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_success_ref_variable() {
    // func: main() { let: x; &x; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None,
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let ref_expr = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::Operation1(
            Operator1::Ref,
            loc_expr(Expression::Variable("x".to_string())),
        ))),
        location: SourceLocation::new(35, 40),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(45, 55),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, ref_expr, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 60),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_error_ref_literal() {
    // func: main() { &5; return:0; }
    let ref_expr = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::Operation1(
            Operator1::Ref,
            loc_expr(Expression::Factor(5)),
        ))),
        location: SourceLocation::new(20, 25),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(30, 40),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![ref_expr, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 45),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_err());

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert_eq!(errors.len(), 1);
    assert!(errors[0]
        .message
        .contains("reference operator (&) can only be applied to variables"));
}

#[test]
fn test_error_ref_expression() {
    // func: main() { let: x; &(x + 1); return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None,
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let ref_expr = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::Operation1(
            Operator1::Ref,
            loc_expr(Expression::Operation2(
                Operator2::Plus,
                loc_expr(Expression::Variable("x".to_string())),
                loc_expr(Expression::Factor(1)),
            )),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, ref_expr, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 65),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_err());

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert_eq!(errors.len(), 1);
    assert!(errors[0]
        .message
        .contains("reference operator (&) can only be applied to variables"));
}

#[test]
fn test_success_deref_variable() {
    // func: main() { let: p; *p; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "p".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None,
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let deref_expr = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::Operation1(
            Operator1::Deref,
            loc_expr(Expression::Variable("p".to_string())),
        ))),
        location: SourceLocation::new(35, 40),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(45, 55),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, deref_expr, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 60),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

// === Array Tests ===

#[test]
fn test_success_array_declaration() {
    // func: main() { let: arr[3]; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            Some(3), // array size
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(35, 45),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 50),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    let func = scope.get_function("__main").unwrap();
    assert_eq!(func.block.scope.variable_count, 3); // 配列は3スロット占有
    assert_eq!(func.block.scope.variables.len(), 1); // 変数自体は1つ
    assert_eq!(func.block.scope.variables[0].array_size, Some(3));
}

#[test]
fn test_success_multiple_variables_with_array() {
    // func: main() { let: a; let: arr[3]; let: b; return:0; }
    let var_a = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "a".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None,
            None,
        ),
        location: SourceLocation::new(20, 25),
    };

    let var_arr = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            Some(3),
            None,
        ),
        location: SourceLocation::new(30, 40),
    };

    let var_b = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "b".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None,
            None,
        ),
        location: SourceLocation::new(45, 50),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(55, 65),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_a, var_arr, var_b, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 70),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    let func = scope.get_function("__main").unwrap();
    assert_eq!(func.block.scope.variable_count, 5); // a(1) + arr(3) + b(1) = 5
    assert_eq!(*func.block.scope.variable_indices.get("a").unwrap(), 0);
    assert_eq!(*func.block.scope.variable_indices.get("arr").unwrap(), 1);
    assert_eq!(*func.block.scope.variable_indices.get("b").unwrap(), 4);
}

#[test]
fn test_success_array_access() {
    // func: main() { let: arr[3]; arr[0]; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            Some(3),
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let array_access = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::ArrayAccess(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, array_access, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 65),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_success_array_assignment() {
    // func: main() { let: arr[3]; arr[0] = 5; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            Some(3),
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let array_assign = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::Operation2(
            Operator2::Assign,
            loc_expr(Expression::ArrayAccess(
                "arr".to_string(),
                loc_expr(Expression::Factor(0)),
            )),
            loc_expr(Expression::Factor(5)),
        ))),
        location: SourceLocation::new(35, 50),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(55, 65),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, array_assign, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 70),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_error_array_access_non_array() {
    // arr[i] は *(&arr + i) と同義。非配列変数へのアクセスも許可される。
    // func: main() { let: x; x[0]; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None, // not an array
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let array_access = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::ArrayAccess(
            "x".to_string(),
            loc_expr(Expression::Factor(0)),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, array_access, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 65),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    // 非配列変数へのインデックスアクセスは合法
    assert!(result.is_ok());
}

#[test]
fn test_error_array_access_undefined() {
    // func: main() { undeclared[0]; return:0; }
    let array_access = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::ArrayAccess(
            "undeclared".to_string(),
            loc_expr(Expression::Factor(0)),
        ))),
        location: SourceLocation::new(20, 35),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(40, 50),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![array_access, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 55),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_err());

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert!(errors[0].message.contains("undefined variable"));
}

#[test]
fn test_success_ref_array_element() {
    // func: main() { let: arr[3]; &arr[0]; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            Some(3),
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let ref_expr = LocatedStatement {
        statement: Statement::Expression(loc_expr(Expression::Operation1(
            Operator1::Ref,
            loc_expr(Expression::ArrayAccess(
                "arr".to_string(),
                loc_expr(Expression::Factor(0)),
            )),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, ref_expr, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 65),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_success_static_array() {
    // func: main() { static: arr[3]; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
            true, // static
            false,
            Some(3),
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(35, 45),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_decl, return_stmt],
            None,
        ),
        location: SourceLocation::new(0, 50),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    let func = scope.get_function("__main").unwrap();
    assert!(func.block.scope.variables[0].is_static);
    assert_eq!(func.block.scope.variables[0].array_size, Some(3));
}

#[test]
fn test_variable_slot_index() {
    // func: main() { let: a; let: arr[3]; let: b; return:0; }
    // slot_index: a=0, arr=1, b=4
    let var_a = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "a".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None,
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let var_arr = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            Some(3),
            None,
        ),
        location: SourceLocation::new(40, 50),
    };

    let var_b = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "b".to_string(),
            loc_expr(Expression::Factor(0)),
            false,
            false,
            None,
            None,
        ),
        location: SourceLocation::new(60, 70),
    };

    let ret = LocatedStatement {
        statement: Statement::Return(Some(loc_expr(Expression::Factor(0)))),
        location: SourceLocation::new(80, 90),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "__main".to_string(),
            vec![],
            vec![var_a, var_arr, var_b, ret],
            None,
        ),
        location: SourceLocation::new(0, 100),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    let func = scope.get_function("__main").unwrap();

    // slot_index が正しく設定されていることを確認
    assert_eq!(func.block.scope.variables[0].slot_index, 0); // a
    assert_eq!(func.block.scope.variables[1].slot_index, 1); // arr
    assert_eq!(func.block.scope.variables[2].slot_index, 4); // b (arr が 3 スロット使用)
}

/// ソースコードを字句解析・構文解析して意味解析する（テスト用）
fn analyze_source(source: &str) -> Result<Scope, Vec<CodeParseError>> {
    let tokens = crate::parse_to_tokens(&source.to_string()).expect("tokenize failed");
    let tree = crate::parse_to_tree(&tokens).expect("parse failed");
    analyze(&tree)
}

#[test]
fn test_block_nested_func_array_access_skips_enclosing_block_array() {
    // ブロック内の関数 g の a[1] は、ブロックの非 static 配列 a[3] ではなくグローバル配列 a[5] を指す。
    // 解決結果（IdentifierRef）と配列サイズが同じ変数のものであることを確認する
    let source = "
        let: a[5];
        func: f() {
          {
            let: a[3];
            func: g() { return: a[1]; }
            return: g();
          };
        }
        func: __main() { __puti(f()); }
    ";
    let scope = analyze_source(source).expect("analyze failed");
    let g = scope.get_function("g").expect("g must be registered");
    let return_expr = match &g.block.statements[..] {
        [LocatedExecStatement {
            statement: ExecStatement::Return(Some(expr)),
            ..
        }] => expr,
        _ => panic!("g must consist of a single return statement"),
    };
    match &return_expr.expression {
        ExecExpression::ArrayAccess(id_ref, _, array_size) => {
            assert!(id_ref.is_global, "a must resolve to the global array");
            assert_eq!(*array_size, 5);
        }
        _ => panic!("return expression must be an array access"),
    }
}

/// 解析のエラーメッセージを 1 つの文字列にまとめる（成功した場合は panic）
fn analyze_error_message(source: &str) -> String {
    match analyze_source(source) {
        Ok(_) => panic!("analysis must fail: {}", source),
        Err(errors) => errors
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[test]
fn test_static_initializer_boundary_excludes_declaring_scope() {
    // 初期化子の境界は宣言したスコープの上にある。宣言したブロック自身の非 static 変数は境界の外
    let message = analyze_error_message("func: f() { { let: a(5); static: s(a); }; }");
    assert!(
        message.contains("cannot reference non-static variable 'a'"),
        "{}",
        message
    );
}

#[test]
fn test_static_initializer_allows_its_own_block_variables_and_statics() {
    // 初期化子の中のブロックの変数は境界の内側。static 変数・グローバル変数は境界の外でも参照できる
    let source = "
        let: g(1);
        func: f() {
          static: a(2);
          { static: b(a + g); static: c({ let: t(3); t + b; }); return: c; };
        }
        func: __main() { __puti(f()); }
    ";
    assert!(analyze_source(source).is_ok());
}

#[test]
fn test_static_initializer_does_not_fall_back_to_same_named_global() {
    // 外の非 static 変数を見つけた時点でエラーにする（外側の同名のグローバル変数に解決しない）
    let message =
        analyze_error_message("let: a(9); func: f() { let: a(1); static: s(a); return: s; }");
    assert!(
        message.contains("cannot reference non-static variable 'a'"),
        "{}",
        message
    );
}

#[test]
fn test_exec_context_new_unit_inside_static_initializer() {
    // 初期化子の中で定義した関数は新しい実行単位: return を書けて、ループの深さも 0 から数える
    let ok = "
        func: f() {
          static: x({ func: g() { let: i(0); while: 1 { i += 1; if: i == 2 { break:; }; }; return: i; } g(); });
          return: x;
        }
        func: __main() { __puti(f()); }
    ";
    assert!(analyze_source(ok).is_ok());
    // 初期化子は外のループの内側でも、ループの深さは 0 から数える
    let message =
        analyze_error_message("func: f() { while: 1 { static: s({ continue:; 0; }); }; }");
    assert!(
        message.contains("continue statement outside of a loop"),
        "{}",
        message
    );
}

#[test]
fn test_exec_context_loop_depth_for_init_and_body() {
    // for の init はループの外、cond / step / body はループの内側
    let message = analyze_error_message(
        "func: f() { for: { let: i(0); break:; } { i < 1; } { i += 1; } { }; }",
    );
    assert!(
        message.contains("break statement outside of a loop"),
        "{}",
        message
    );
    let ok = "func: f() { for: { let: i(0); } { i < 1; } { i += 1; } { break:; }; } func: __main() { f(); }";
    assert!(analyze_source(ok).is_ok());
}

/// 初期化文 `x = ...`（`Expression(Assign(Variable(ref), ...))` / 配列要素への代入）の代入先のグローバル領域のオフセット
fn static_init_target_offset(stmt: &LocatedExecStatement) -> usize {
    let ExecStatement::Expression(expr) = &stmt.statement else {
        panic!("static initializer must be an expression statement");
    };
    let ExecExpression::Operation2(Operator2::Assign, lhs, _) = &expr.expression else {
        panic!("static initializer must be an assignment");
    };
    match &lhs.expression {
        ExecExpression::Variable(id_ref, _) | ExecExpression::ArrayAccess(id_ref, _, _) => {
            assert!(
                id_ref.is_global,
                "static variables must be in the global region"
            );
            id_ref.local_index
        }
        _ => panic!("assignment target must be a variable"),
    }
}

#[test]
fn test_block_static_is_in_global_region() {
    // ブロックの static は is_global の参照になり、ブロックのフレームのスロットを消費しない
    let source = "func: f() { { static: s(3); return: s; }; } func: __main() { __puti(f()); }";
    let scope = analyze_source(source).expect("analyze failed");
    let f = scope.get_function("f").expect("f must exist");
    let block = match &f.block.statements[0].statement {
        ExecStatement::Expression(expr) => match &expr.expression {
            ExecExpression::Block(block) => block,
            _ => panic!("expected a block expression"),
        },
        _ => panic!("expected an expression statement"),
    };
    assert_eq!(block.scope.variable_count, 0);
    match &block.statements[0].statement {
        ExecStatement::Return(Some(expr)) => match &expr.expression {
            ExecExpression::Variable(id_ref, _) => {
                assert!(id_ref.is_global);
                assert_eq!(id_ref.local_index, 0);
            }
            _ => panic!("expected a variable"),
        },
        _ => panic!("expected a return statement"),
    }
    // グローバル領域 = ルートの変数 0 個 + ブロックの static 1 個
    assert_eq!(scope.variable_count, 1);
}

#[test]
fn test_function_statics_do_not_use_frame_slots() {
    let source = "let: g; func: f() { static: a(1); let: b; b = a; return: b; } func: __main() { __puti(f()); }";
    let scope = analyze_source(source).expect("analyze failed");
    let f = scope.get_function("f").expect("f must exist");
    assert_eq!(f.block.scope.variable_count, 1, "only b uses a frame slot");
    assert_eq!(scope.variable_count, 2, "g and the static a");
    assert!(f.block.scope.static_init_statements.is_empty());
}

#[test]
fn test_static_init_statements_are_in_source_order() {
    // r（ルート、オフセット 0）→ f の a → ブロックの b → c の順に、ルートの 1 つの列に入る。
    // オフセットは宣言の走査順に割り当てられる（f の関数スコープの a・c が先、ブロックの b が後）
    let source = "
        static: r(0);
        func: f() { static: a(1); { static: b(2); }; static: c(3); }
        func: __main() { f(); }
    ";
    let scope = analyze_source(source).expect("analyze failed");
    let offsets: Vec<usize> = scope
        .static_init_statements
        .iter()
        .map(static_init_target_offset)
        .collect();
    assert_eq!(offsets, vec![0, 1, 3, 2]);
}

#[test]
fn test_static_declared_in_initializer_is_initialized_first() {
    // static: x({ static: y(4); y + 1; }) の y の初期化文は x より前
    let source = "func: f() { static: x({ static: y(4); y + 1; }); return: x; } func: __main() { __puti(f()); }";
    let scope = analyze_source(source).expect("analyze failed");
    let offsets: Vec<usize> = scope
        .static_init_statements
        .iter()
        .map(static_init_target_offset)
        .collect();
    // x は関数スコープの走査で先に割り当てられ（0）、y は初期化子のブロックの解析で割り当てられる（1）
    assert_eq!(offsets, vec![1, 0]);
}
