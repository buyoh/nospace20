//! Semantic Analyzer のテストコード

use super::*;
use crate::base::SourceLocation;

#[test]
fn test_error_return_outside_function() {
    // return:0; at root level should error with position
    let statements = vec![LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
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
    // Phase 3: グローバル変数の初期化式を許可
    let statements = vec![LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::Factor(42))),
        location: SourceLocation::new(50, 55),
    }];

    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_error_nested_function_declaration() {
    // func: outer() { func: inner() {} }
    let inner_func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "inner".to_string(),
            vec![],
            vec![], // empty body
        ),
        location: SourceLocation::new(100, 120),
    };

    let outer_func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "outer".to_string(),
            vec![],
            vec![inner_func],
        ),
        location: SourceLocation::new(80, 130),
    };

    let statements = vec![outer_func];
    let result = analyze(&statements);
    assert!(result.is_err());

    let errors = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code_pointer, Some(100));
    assert!(errors[0]
        .message
        .contains("nested function declaration is not supported"));
}

#[test]
fn test_success_block_scoped_variable() {
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            Box::new(Expression::Factor(0)),
            false, // non-static
            None,  // not an array
        ),
        location: SourceLocation::new(150, 160),
    };

    // ブロックスコープでの変数宣言をシミュレート
    // If式のthen節内で変数宣言を試みる
    let if_expr = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::If(
            Box::new(Expression::Factor(1)),
            vec![var_decl], // block内の変数宣言
            vec![],
        ))),
        location: SourceLocation::new(140, 170),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration("test".to_string(), vec![], vec![if_expr]),
        location: SourceLocation::new(135, 175),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_success_global_variable() {
    // Phase 3: グローバル変数を許可
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "global".to_string(),
            Box::new(Expression::Factor(42)),
            false, // non-static explicitly, but global is implicitly static
            None,  // not an array
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
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(20, 30),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![return_stmt],
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
            Box::new(Expression::Factor(0)),
            false,
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let ref_expr = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::Operation1(
            Operator1::Ref,
            Box::new(Expression::Variable("x".to_string())),
        ))),
        location: SourceLocation::new(35, 40),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(45, 55),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, ref_expr, return_stmt],
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
        statement: Statement::Expression(Box::new(Expression::Operation1(
            Operator1::Ref,
            Box::new(Expression::Factor(5)),
        ))),
        location: SourceLocation::new(20, 25),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(30, 40),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![ref_expr, return_stmt],
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
            Box::new(Expression::Factor(0)),
            false,
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let ref_expr = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::Operation1(
            Operator1::Ref,
            Box::new(Expression::Operation2(
                Operator2::Plus,
                Box::new(Expression::Variable("x".to_string())),
                Box::new(Expression::Factor(1)),
            )),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, ref_expr, return_stmt],
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
            Box::new(Expression::Factor(0)),
            false,
            None,
        ),
        location: SourceLocation::new(20, 30),
    };

    let deref_expr = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::Operation1(
            Operator1::Deref,
            Box::new(Expression::Variable("p".to_string())),
        ))),
        location: SourceLocation::new(35, 40),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(45, 55),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, deref_expr, return_stmt],
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
            Box::new(Expression::Factor(0)),
            false,
            Some(3), // array size
        ),
        location: SourceLocation::new(20, 30),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(35, 45),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, return_stmt],
        ),
        location: SourceLocation::new(0, 50),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    let func = scope.get_function("main").unwrap();
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
            Box::new(Expression::Factor(0)),
            false,
            None,
        ),
        location: SourceLocation::new(20, 25),
    };

    let var_arr = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "arr".to_string(),
            Box::new(Expression::Factor(0)),
            false,
            Some(3),
        ),
        location: SourceLocation::new(30, 40),
    };

    let var_b = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "b".to_string(),
            Box::new(Expression::Factor(0)),
            false,
            None,
        ),
        location: SourceLocation::new(45, 50),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(55, 65),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_a, var_arr, var_b, return_stmt],
        ),
        location: SourceLocation::new(0, 70),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    let func = scope.get_function("main").unwrap();
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
            Box::new(Expression::Factor(0)),
            false,
            Some(3),
        ),
        location: SourceLocation::new(20, 30),
    };

    let array_access = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::ArrayAccess(
            "arr".to_string(),
            Box::new(Expression::Factor(0)),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, array_access, return_stmt],
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
            Box::new(Expression::Factor(0)),
            false,
            Some(3),
        ),
        location: SourceLocation::new(20, 30),
    };

    let array_assign = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::Operation2(
            Operator2::Assign,
            Box::new(Expression::ArrayAccess(
                "arr".to_string(),
                Box::new(Expression::Factor(0)),
            )),
            Box::new(Expression::Factor(5)),
        ))),
        location: SourceLocation::new(35, 50),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(55, 65),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, array_assign, return_stmt],
        ),
        location: SourceLocation::new(0, 70),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());
}

#[test]
fn test_error_array_access_non_array() {
    // func: main() { let: x; x[0]; return:0; }
    let var_decl = LocatedStatement {
        statement: Statement::VariableDeclaration(
            "x".to_string(),
            Box::new(Expression::Factor(0)),
            false,
            None, // not an array
        ),
        location: SourceLocation::new(20, 30),
    };

    let array_access = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::ArrayAccess(
            "x".to_string(),
            Box::new(Expression::Factor(0)),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, array_access, return_stmt],
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
    assert!(errors[0].message.contains("is not an array"));
}

#[test]
fn test_error_array_access_undefined() {
    // func: main() { undeclared[0]; return:0; }
    let array_access = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::ArrayAccess(
            "undeclared".to_string(),
            Box::new(Expression::Factor(0)),
        ))),
        location: SourceLocation::new(20, 35),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(40, 50),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![array_access, return_stmt],
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
            Box::new(Expression::Factor(0)),
            false,
            Some(3),
        ),
        location: SourceLocation::new(20, 30),
    };

    let ref_expr = LocatedStatement {
        statement: Statement::Expression(Box::new(Expression::Operation1(
            Operator1::Ref,
            Box::new(Expression::ArrayAccess(
                "arr".to_string(),
                Box::new(Expression::Factor(0)),
            )),
        ))),
        location: SourceLocation::new(35, 45),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(50, 60),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, ref_expr, return_stmt],
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
            Box::new(Expression::Factor(0)),
            true, // static
            Some(3),
        ),
        location: SourceLocation::new(20, 30),
    };

    let return_stmt = LocatedStatement {
        statement: Statement::Return(Box::new(Expression::Factor(0))),
        location: SourceLocation::new(35, 45),
    };

    let func = LocatedStatement {
        statement: Statement::FunctionDeclaration(
            "main".to_string(),
            vec![],
            vec![var_decl, return_stmt],
        ),
        location: SourceLocation::new(0, 50),
    };

    let statements = vec![func];
    let result = analyze(&statements);
    assert!(result.is_ok());

    let scope = result.unwrap();
    let func = scope.get_function("main").unwrap();
    assert!(func.block.scope.variables[0].is_static);
    assert_eq!(func.block.scope.variables[0].array_size, Some(3));
}
