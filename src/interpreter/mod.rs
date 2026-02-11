//! # Interpreter
//!
//! コンパイル前のコードを実行します。
//! コンパイラの実装は多様で複雑になりがちな為、Interpreterは極力シンプルな実装となるよう
//! 他のモジュールを設計しなければなりません。
//!

mod environment;
mod exec;
mod types;

pub use environment::{Environment, EnvironmentConfig};
#[allow(unused_imports)]
pub use environment::EnvironmentMetrics;

use crate::semantic_analyzer::Scope;
use exec::LocalEnvironment;
use types::Flow;

pub fn interpret_func(env: &mut Environment, scope: &Scope, func_name: &str) -> Option<i64> {
    let func = match scope.get_function(func_name) {
        Some(f) => f,
        None => {
            eprintln!("error: function '{}' not found", func_name);
            return None;
        }
    };
    let mut e = LocalEnvironment::new_func(env, scope, &func, &Vec::<i64>::new());
    let res = e.interpret_statements(&func.block.statements);
    if let Flow::Return(x) = res {
        Some(x)
    } else {
        None
    }
}

/// グローバル変数の領域確保と初期化式の実行
///
/// 初期化順序:
/// 1. static 変数の初期化式を実行（ルートレベル）
/// 2. 関数内 static 変数の初期化式を実行
/// 3. 非 static グローバル変数の初期化式を実行
pub fn interpret_global(env: &mut Environment, scope: &Scope) {
    // グローバル変数の領域を確保
    env.global_variables = vec![0; scope.variable_count];

    // ルートレベル static 変数の初期化式を先に実行
    if !scope.static_init_statements.is_empty() {
        let mut local_env = LocalEnvironment {
            env,
            root_scope: scope,
            scope_stack: Vec::new(),
        };
        for statement in &scope.static_init_statements {
            match local_env.interpret_statement(statement) {
                Flow::Proceed => (),
                other => panic!("unexpected flow in static initialization: {:?}", other),
            }
        }
    }

    // 関数内 static 変数の初期化
    initialize_function_statics(env, scope);

    // 非 static グローバル変数の初期化式を実行
    if !scope.root_statements.is_empty() {
        let mut local_env = LocalEnvironment {
            env,
            root_scope: scope,
            scope_stack: Vec::new(),
        };
        for statement in &scope.root_statements {
            match local_env.interpret_statement(statement) {
                Flow::Proceed => (),
                other => panic!("unexpected flow in global initialization: {:?}", other),
            }
        }
    }
}

/// 関数内 static 変数の初期化
///
/// 全関数をスキャンし、static 変数を持つ関数について永続ストレージを作成する。
/// static 変数の初期化式がある場合は、一時的なスコープで実行して初期値を設定する。
fn initialize_function_statics(env: &mut Environment, scope: &Scope) {
    for name in &scope.function_names {
        let func = match scope.get_function(name) {
            Some(f) => f,
            None => continue,
        };
        let has_static = func.block.scope.variables.iter().any(|v| v.is_static);
        if !has_static {
            continue;
        }

        let storage = if !func.block.scope.static_init_statements.is_empty() {
            // static 変数の初期化式を一時的なスコープで実行
            let init_storage = vec![0i64; func.block.scope.variable_count];
            let mut local_env = LocalEnvironment {
                env: &mut *env,
                root_scope: scope,
                scope_stack: vec![init_storage],
            };
            for stmt in &func.block.scope.static_init_statements {
                match local_env.interpret_statement(stmt) {
                    Flow::Proceed => (),
                    other => panic!("unexpected flow in function static initialization: {:?}", other),
                }
            }
            local_env.scope_stack.pop().unwrap()
        } else {
            // 初期化式なし: デフォルト値（全て0）
            vec![0i64; func.block.scope.variable_count]
        };

        env.function_static_storage.insert(name.clone(), storage);
    }
}

/// グローバル変数を初期化してから main 関数を実行
pub fn interpret_all(env: &mut Environment, scope: &Scope) -> Option<i64> {
    interpret_global(env, scope);
    interpret_func(env, scope, "main")
}
