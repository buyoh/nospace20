#[cfg(test)]
#[macro_use]
extern crate assert_matches;

use std::collections::BTreeMap;

pub use base::CodeParseError;
pub use interpreter::{Environment, EnvironmentConfig};
pub use logger::TextCode;
pub use semantic_analyzer::Scope;
use token_parser::PrettyToken;
use tree_parser::LocatedStatement;

mod base;
mod compile_property;
mod compiler_ws;
mod interpreter;
mod logger;
mod semantic_analyzer;
mod token_parser;
mod tree_parser;
pub mod whitespace;

#[cfg(feature = "wasm")]
mod wasm_api;

pub use compile_property::{
    CompileProperty, CompileTarget, ExecutionMode, LanguageStd, TargetExtension,
};

pub fn parse_to_tokens(text: &String) -> Result<Vec<PrettyToken>, Vec<CodeParseError>> {
    match token_parser::parse_to_tokens(text) {
        Ok(x) => Ok(x),
        Err(err) => Err(err),
    }
}

pub fn parse_to_tree(
    tokens: &Vec<PrettyToken>,
) -> Result<Vec<LocatedStatement>, Vec<CodeParseError>> {
    match tree_parser::parse_to_tree(tokens) {
        Ok(x) => Ok(x),
        Err(err) => Err(err),
    }
}

pub fn syntactic_analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    semantic_analyzer::analyze(root)
}

/// グローバル変数の初期化を含む interpret
pub fn interpret(scope: &Scope) -> Option<i64> {
    let mut env = Environment::new();
    interpreter::interpret_all(&mut env, scope)
}

/// グローバル変数の初期化を含む interpret（env 指定版）
pub fn interpret_with_env(env: &mut Environment, scope: &Scope) -> Option<i64> {
    interpreter::interpret_all(env, scope)
}

pub fn interpret_func(scope: &Scope, func_name: &str) -> Option<i64> {
    let mut env = Environment::new();
    interpreter::interpret_func(&mut env, scope, func_name)
}

pub fn interpret_func_with_env(
    env: &mut Environment,
    scope: &Scope,
    func_name: &str,
) -> Option<i64> {
    interpreter::interpret_func(env, scope, func_name)
}

pub fn interpret_func_testing(scope: &Scope, func_name: &str) -> BTreeMap<i64, i64> {
    use std::io::Cursor;
    // テスト用には空のバッファを使用
    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(Vec::<u8>::new())));
    let stdout_buf: Box<dyn std::io::Write> = Box::new(Vec::<u8>::new());
    let config = EnvironmentConfig::with_max_expression_count(100000);
    let mut env = Environment::new_with_config(stdin_cursor, stdout_buf, config);

    // グローバル変数を初期化してから関数を実行
    interpreter::interpret_global(&mut env, scope);
    interpreter::interpret_func(&mut env, scope, func_name);
    env.traced
}

/// テスト用インタプリタ実行（randomize_uninit モード）
///
/// 未初期化変数にランダム値を設定して実行する。初期値 0 依存のバグを検出するために使用する。
pub fn interpret_func_testing_randomize(scope: &Scope, func_name: &str) -> BTreeMap<i64, i64> {
    use std::io::Cursor;
    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(Vec::<u8>::new())));
    let stdout_buf: Box<dyn std::io::Write> = Box::new(Vec::<u8>::new());
    let mut config = EnvironmentConfig::with_max_expression_count(100000);
    config.randomize_uninit = true;
    let mut env = Environment::new_with_config(stdin_cursor, stdout_buf, config);

    interpreter::interpret_global(&mut env, scope);
    interpreter::interpret_func(&mut env, scope, func_name);
    env.traced
}

pub fn interpret_func_with_io(
    scope: &Scope,
    func_name: &str,
    stdin: &str,
) -> (BTreeMap<i64, i64>, String) {
    use std::cell::RefCell;
    use std::io::Cursor;
    use std::rc::Rc;

    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(
        stdin.as_bytes().to_vec(),
    )));

    // Rc<RefCell<Vec<u8>>> を使ってstdoutを共有
    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let stdout_clone = Rc::clone(&stdout_buf);

    struct SharedWriter(Rc<RefCell<Vec<u8>>>);
    impl std::io::Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.borrow_mut().flush()
        }
    }

    let stdout_writer: Box<dyn std::io::Write> = Box::new(SharedWriter(stdout_clone));
    let config = EnvironmentConfig::with_max_expression_count(100000);
    let mut env = Environment::new_with_config(stdin_cursor, stdout_writer, config);

    // グローバル変数を初期化してから関数を実行
    interpreter::interpret_global(&mut env, scope);
    interpreter::interpret_func(&mut env, scope, func_name);
    env.flush();

    let stdout_vec = stdout_buf.borrow().clone();
    let stdout_string = String::from_utf8(stdout_vec).unwrap();

    (env.traced, stdout_string)
}

/// Whitespace にコンパイル（拡張オプション付き）
pub fn compile_to_whitespace_with_options(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
) -> Result<String, String> {
    compiler_ws::compile_with_options(scope, debug_ext, alloc_ext)
        .map(|prog| prog.to_whitespace())
        .map_err(|e| e.to_string())
}

/// Whitespace にコンパイル（デバッグ用ニーモニック、拡張オプション付き）
pub fn compile_to_whitespace_debug_with_options(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
) -> Result<String, String> {
    compiler_ws::compile_with_options(scope, debug_ext, alloc_ext)
        .map(|prog| prog.to_debug_string())
        .map_err(|e| e.to_string())
}

/// Whitespace にコンパイル（従来互換）
pub fn compile_to_whitespace(scope: &Scope) -> Result<String, String> {
    compile_to_whitespace_with_options(scope, false, false)
}

/// Whitespace にコンパイル（デバッグ用ニーモニック、従来互換）
pub fn compile_to_whitespace_debug(scope: &Scope) -> Result<String, String> {
    compile_to_whitespace_debug_with_options(scope, false, false)
}
