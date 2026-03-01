//! # Interpreter
//!
//! コンパイル前のコードを実行します。
//! コンパイラの実装は多様で複雑になりがちな為、Interpreterは極力シンプルな実装となるよう
//! 他のモジュールを設計しなければなりません。
//!

mod environment;
mod exec;
mod types;

#[allow(unused_imports)]
pub use environment::EnvironmentMetrics;
pub use environment::{Environment, EnvironmentConfig};

use crate::semantic_analyzer::Scope;
use exec::LocalEnvironment;
use types::Flow;

// InterpretError を base::error から re-export（後方互換）
pub use crate::base::error::interpret_error::InterpretError;

pub fn interpret_func(env: &mut Environment, scope: &Scope, func_name: &str) -> Result<Option<i64>, InterpretError> {
    let func = match scope.get_function(func_name) {
        Some(f) => f,
        None => {
            return Err(InterpretError::FunctionNotFound(func_name.to_string()));
        }
    };
    let mut e = LocalEnvironment::new_func(env, scope, &func, &Vec::<i64>::new());
    let res = e.interpret_statements(&func.block.statements);
    if let Flow::Return(x) = res {
        Ok(Some(x))
    } else {
        Ok(None)
    }
}

/// グローバル変数の領域確保と初期化式の実行
///
/// 初期化順序:
/// 1. static 変数の初期化式を実行（ルートレベル）
/// 2. 関数内 static 変数の初期化式を実行
/// 3. 非 static グローバル変数の初期化式を実行
pub fn interpret_global(env: &mut Environment, scope: &Scope) -> Result<(), InterpretError> {
    // グローバル変数の領域を確保（randomize_uninit モードではランダム値で初期化）
    env.global_variables = exec::create_uninit_vec(scope.variable_count, env.config.randomize_uninit);

    // ルートレベル static 変数の初期化式を先に実行
    if !scope.static_init_statements.is_empty() {
        let mut local_env = LocalEnvironment {
            env,
            root_scope: scope,
            scope_stack: Vec::new(),
        };
        for located_stmt in &scope.static_init_statements {
            match local_env.interpret_statement(&located_stmt.statement) {
                Flow::Proceed => (),
                other => {
                    return Err(InterpretError::UnexpectedFlow(format!(
                        "in static initialization: {:?}",
                        other
                    )));
                }
            }
        }
    }

    // 関数内 static 変数の初期化
    initialize_function_statics(env, scope)?;

    // 非 static グローバル変数の初期化式を実行
    if !scope.root_statements.is_empty() {
        let mut local_env = LocalEnvironment {
            env,
            root_scope: scope,
            scope_stack: Vec::new(),
        };
        for located_stmt in &scope.root_statements {
            match local_env.interpret_statement(&located_stmt.statement) {
                Flow::Proceed => (),
                other => {
                    return Err(InterpretError::UnexpectedFlow(format!(
                        "in global initialization: {:?}",
                        other
                    )));
                }
            }
        }
    }

    Ok(())
}

/// 関数内 static 変数の初期化
///
/// 全関数をスキャンし、static 変数を持つ関数について永続ストレージを作成する。
/// static 変数の初期化式がある場合は、一時的なスコープで実行して初期値を設定する。
fn initialize_function_statics(env: &mut Environment, scope: &Scope) -> Result<(), InterpretError> {
    // Phase 6: インデックスベースで関数にアクセス
    for (func_idx, func) in scope.functions.iter().enumerate() {
        let has_static = func.block.scope.variables.iter().any(|v| v.is_static);
        if !has_static {
            continue;
        }

        let storage = if !func.block.scope.static_init_statements.is_empty() {
            // static 変数の初期化式を一時的なスコープで実行
            let init_storage = exec::create_uninit_vec(
                func.block.scope.variable_count,
                env.config.randomize_uninit,
            );
            let mut local_env = LocalEnvironment {
                env: &mut *env,
                root_scope: scope,
                scope_stack: vec![init_storage],
            };
            for stmt in &func.block.scope.static_init_statements {
                match local_env.interpret_statement(&stmt.statement) {
                    Flow::Proceed => (),
                    other => {
                        return Err(InterpretError::UnexpectedFlow(format!(
                            "in function static initialization: {:?}",
                            other
                        )));
                    }
                }
            }
            local_env.scope_stack.pop().unwrap()
        } else {
            // 初期化式なし: randomize_uninit モードではランダム値、それ以外は 0
            exec::create_uninit_vec(
                func.block.scope.variable_count,
                env.config.randomize_uninit,
            )
        };

        // Phase 6: 関数インデックスをキーとして使用
        env.function_static_storage.insert(func_idx, storage);
    }

    Ok(())
}

/// グローバル変数を初期化してから __main 関数を実行
pub fn interpret_all(env: &mut Environment, scope: &Scope) -> Result<Option<i64>, InterpretError> {
    interpret_global(env, scope)?;
    // Phase 6: main_function_index を使用してインデックスベースでアクセス
    if let Some(main_idx) = scope.main_function_index {
        let func = &scope.functions[main_idx];
        let mut e = LocalEnvironment::new_func(env, scope, func, &Vec::<i64>::new());
        let res = e.interpret_statements(&func.block.statements);
        if let Flow::Return(x) = res {
            Ok(Some(x))
        } else {
            Ok(None)
        }
    } else {
        Err(InterpretError::FunctionNotFound("__main".to_string()))
    }
}
