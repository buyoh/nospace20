//! # Interpreter
//!
//! コンパイル前のコードを実行します。
//! コンパイラの実装は多様で複雑になりがちな為、Interpreterは極力シンプルな実装となるよう
//! 他のモジュールを設計しなければなりません。
//!

mod types;
mod environment;
mod exec;

pub use environment::{Environment, EnvironmentConfig, EnvironmentMetrics};

use crate::semantic_analyzer::Scope;
use exec::LocalEnvironment;
use types::Flow;

pub fn interpret_func(env: &mut Environment, scope: &Scope, func_name: &str) -> Option<i64> {
    let func = scope.get_function(func_name).unwrap();
    let mut e = LocalEnvironment::new_func(env, scope, &func, &Vec::<i64>::new());
    let res = e.interpret_statements(&func.block.statements);
    if let Flow::Return(x) = res {
        Some(x)
    } else {
        None
    }
}

/// Phase 3: グローバル変数を初期化してから main 関数を実行
pub fn interpret(env: &mut Environment, scope: &Scope) -> Option<i64> {
    // グローバル変数の領域を確保
    env.global_variables = vec![0; scope.variable_count];

    // グローバル変数の初期化式を実行
    // ルートスコープの実行文を実行（ローカルスコープなしで実行）
    if !scope.root_statements.is_empty() {
        let mut local_env = LocalEnvironment {
            env,
            root_scope: scope,
            scope_stack: Vec::new(), // グローバル変数の初期化時はローカルスコープなし
        };

        for statement in &scope.root_statements {
            match local_env.interpret_statement(statement) {
                Flow::Proceed => (),
                // グローバル変数の初期化で return/break/continue は発生しないはず
                other => panic!("unexpected flow in global initialization: {:?}", other),
            }
        }
    }

    // main 関数を呼び出し
    interpret_func(env, scope, "main")
}
