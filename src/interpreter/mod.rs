//! # Interpreter
//!
//! コンパイル前のコードを実行します。
//! コンパイラの実装は多様で複雑になりがちな為、Interpreterは極力シンプルな実装となるよう
//! 他のモジュールを設計しなければなりません。
//!

mod allocator;
mod environment;
mod exec;
mod static_storage;
mod types;
pub mod vm;

#[allow(unused_imports)]
pub use environment::EnvironmentMetrics;
pub use environment::{Environment, EnvironmentConfig};
pub use vm::{NospaceVM, StepResult};

use crate::semantic_analyzer::Scope;
use exec::LocalEnvironment;
use types::Flow;

// InterpretError を base::error から re-export（後方互換）
pub use crate::base::error::interpret_error::InterpretError;

pub fn interpret_func(
    env: &mut Environment,
    scope: &Scope,
    func_name: &str,
) -> Result<Option<i64>, InterpretError> {
    let func_idx = match scope.get_function_index(func_name) {
        Some(i) => i,
        None => {
            return Err(InterpretError::FunctionNotFound(func_name.to_string()));
        }
    };
    let func = &scope.functions[func_idx];
    let mut e = LocalEnvironment::new_func(env, scope, func_idx, &Vec::<i64>::new());
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
    // グローバル変数の領域をアロケータで確保（randomize_uninit モードではランダム値で初期化）
    env.global_base_addr = env
        .allocator
        .alloc_internal_uninit(scope.variable_count, env.config.randomize_uninit);

    // 全関数の static 領域を、どの初期化式よりも先に確保する（NospaceVM の AllocGlobals と同じ順序）
    // バグ修正: 従来は関数ごとに「static 領域の確保 → 初期化式の実行」を行っていたため、
    // ルートや先に初期化される関数の初期化式から後ろの関数を呼ぶと、その関数の static 領域が無かった。
    // 解決策: 初期化式の実行前に全関数の領域を確保する。
    env.function_statics = static_storage::FunctionStaticStorage::allocate(
        scope,
        &mut env.allocator,
        env.config.randomize_uninit,
    );

    // ルートレベル static 変数の初期化式を先に実行
    if !scope.static_init_statements.is_empty() {
        let mut local_env = LocalEnvironment {
            env,
            root_scope: scope,
            scope_stack: Vec::new(),
            func_frames: Vec::new(),
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
            func_frames: Vec::new(),
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
/// static 領域（`interpret_global` で確保済み）の上で、各関数の static 変数の初期化式を実行する。
fn initialize_function_statics(env: &mut Environment, scope: &Scope) -> Result<(), InterpretError> {
    for (func_idx, func) in scope.functions.iter().enumerate() {
        if func.block.scope.static_init_statements.is_empty() {
            continue;
        }
        // static 初期化式を持つ関数は static 変数を持つので、allocate で領域を確保済み（内部の不変条件）
        let static_addr = env
            .function_statics
            .base_addr(func_idx)
            .expect("internal error: static storage is not allocated");

        // static_addr ブロック上で直接初期化式を実行
        let mut local_env = LocalEnvironment {
            env: &mut *env,
            root_scope: scope,
            scope_stack: vec![static_addr],
            func_frames: Vec::new(),
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
    }

    Ok(())
}

/// グローバル変数を初期化してから __main 関数を実行
pub fn interpret_all(env: &mut Environment, scope: &Scope) -> Result<Option<i64>, InterpretError> {
    interpret_global(env, scope)?;
    // Phase 6: main_function_index を使用してインデックスベースでアクセス
    if let Some(main_idx) = scope.main_function_index {
        let func = &scope.functions[main_idx];
        let mut e = LocalEnvironment::new_func(env, scope, main_idx, &Vec::<i64>::new());
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
