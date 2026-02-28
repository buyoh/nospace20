#[cfg(test)]
#[macro_use]
extern crate assert_matches;

use std::collections::BTreeMap;

pub use base::CodeParseError;
pub use interpreter::{Environment, EnvironmentConfig, InterpretError};
pub use logger::TextCode;
pub use semantic_analyzer::Scope;
use token_parser::PrettyToken;
use tree_parser::LocatedStatement;

mod base;
mod compile_property;
pub mod compiler_ws;
mod interpreter;
mod logger;
pub mod optimizer;
mod semantic_analyzer;
mod token_parser;
mod tree_parser;
pub mod whitespace;

#[cfg(feature = "cli")]
pub mod cli_utils;

#[cfg(feature = "wasm")]
mod wasm_api;

pub use compile_property::{
    CompileProperty, CompileTarget, ExecutionMode, LanguageStd, TargetExtension, ValidationError,
};
pub use optimizer::OptimizationOptions;

pub fn parse_to_tokens(text: &String) -> Result<Vec<PrettyToken>, Vec<CodeParseError>> {
    token_parser::parse_to_tokens(text)
}

pub fn parse_to_tree(
    tokens: &Vec<PrettyToken>,
) -> Result<Vec<LocatedStatement>, Vec<CodeParseError>> {
    tree_parser::parse_to_tree(tokens)
}

/// 意味解析を実行し、スコープツリーを構築する
pub fn semantic_analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    semantic_analyzer::analyze(root)
}

/// `semantic_analyze` の旧名称（後方互換性のため残存）
#[deprecated(since = "0.2.0", note = "Renamed to semantic_analyze")]
pub fn syntactic_analyze(root: &Vec<LocatedStatement>) -> Result<Scope, Vec<CodeParseError>> {
    semantic_analyze(root)
}

/// Scope に対して最適化パスを適用する
pub fn optimize(scope: &mut Scope, options: &OptimizationOptions) {
    optimizer::optimize(scope, options);
}

/// グローバル変数の初期化を含む interpret
pub fn interpret(scope: &Scope) -> Result<Option<i64>, InterpretError> {
    let mut env = Environment::new();
    interpreter::interpret_all(&mut env, scope)
}

/// グローバル変数の初期化を含む interpret（env 指定版）
pub fn interpret_with_env(env: &mut Environment, scope: &Scope) -> Result<Option<i64>, InterpretError> {
    interpreter::interpret_all(env, scope)
}

pub fn interpret_func(scope: &Scope, func_name: &str) -> Result<Option<i64>, InterpretError> {
    let mut env = Environment::new();
    interpreter::interpret_global(&mut env, scope)?;
    interpreter::interpret_func(&mut env, scope, func_name)
}

pub fn interpret_func_with_env(
    env: &mut Environment,
    scope: &Scope,
    func_name: &str,
) -> Result<Option<i64>, InterpretError> {
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
    interpreter::interpret_global(&mut env, scope).expect("global initialization failed");
    let _ = interpreter::interpret_func(&mut env, scope, func_name);
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

    interpreter::interpret_global(&mut env, scope).expect("global initialization failed");
    let _ = interpreter::interpret_func(&mut env, scope, func_name);
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
    interpreter::interpret_global(&mut env, scope).expect("global initialization failed");
    let _ = interpreter::interpret_func(&mut env, scope, func_name);
    env.flush();

    let stdout_vec = stdout_buf.borrow().clone();
    let stdout_string = String::from_utf8(stdout_vec).unwrap();

    (env.traced, stdout_string)
}

/// Whitespace コンパイル出力形式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WsOutputFormat {
    /// Whitespace コード（空白文字のみ）
    #[default]
    Whitespace,
    /// デバッグ用ニーモニック
    Mnemonic,
}

/// Whitespace コンパイルオプション
///
/// `compile_to_ws` 関数に渡す統合オプション構造体。
/// 従来の複数の `compile_to_whitespace_*` 関数を一つに統合する。
#[derive(Debug, Clone, Default)]
pub struct WsCompileOptions {
    /// デバッグ拡張 API を有効化
    pub debug_ext: bool,
    /// メモリアロケータ拡張を有効化
    pub alloc_ext: bool,
    /// 出力形式
    pub output_format: WsOutputFormat,
    /// 最適化オプション
    pub optimization: OptimizationOptions,
}

/// Whitespace にコンパイル（統合 API）
///
/// `WsCompileOptions` によって出力形式・拡張・最適化を一括指定できる。
pub fn compile_to_ws(
    scope: &Scope,
    options: &WsCompileOptions,
) -> Result<String, Vec<CodeParseError>> {
    let prog = compiler_ws::compile_with_full_options(
        scope,
        options.debug_ext,
        options.alloc_ext,
        options.optimization.peephole,
    )
    .map_err(|e| vec![compile_error_to_code_parse_error(e)])?;

    match options.output_format {
        WsOutputFormat::Whitespace => Ok(prog.to_whitespace()),
        WsOutputFormat::Mnemonic => Ok(prog.to_debug_string()),
    }
}

/// Whitespace にコンパイル（拡張オプション付き）
#[deprecated(since = "0.2.0", note = "Use compile_to_ws with WsCompileOptions instead")]
pub fn compile_to_whitespace_with_options(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
) -> Result<String, Vec<CodeParseError>> {
    compiler_ws::compile_with_options(scope, debug_ext, alloc_ext)
        .map(|prog| prog.to_whitespace())
        .map_err(|e| vec![compile_error_to_code_parse_error(e)])
}

/// Whitespace にコンパイル（最適化オプション付き）
///
/// `OptimizationOptions` の `peephole` フラグによって
/// ピープホール最適化を追加適用できる。
#[deprecated(since = "0.2.0", note = "Use compile_to_ws with WsCompileOptions instead")]
pub fn compile_to_whitespace_with_opt(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
    opt: &OptimizationOptions,
) -> Result<String, Vec<CodeParseError>> {
    compiler_ws::compile_with_full_options(scope, debug_ext, alloc_ext, opt.peephole)
        .map(|prog| prog.to_whitespace())
        .map_err(|e| vec![compile_error_to_code_parse_error(e)])
}

/// Whitespace にコンパイル（デバッグ用ニーモニック、拡張オプション付き）
#[deprecated(since = "0.2.0", note = "Use compile_to_ws with WsCompileOptions instead")]
pub fn compile_to_whitespace_debug_with_options(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
) -> Result<String, Vec<CodeParseError>> {
    compiler_ws::compile_with_options(scope, debug_ext, alloc_ext)
        .map(|prog| prog.to_debug_string())
        .map_err(|e| vec![compile_error_to_code_parse_error(e)])
}

/// Whitespace にコンパイル（デバッグ用ニーモニック、最適化オプション付き）
#[deprecated(since = "0.2.0", note = "Use compile_to_ws with WsCompileOptions instead")]
pub fn compile_to_whitespace_debug_with_opt(
    scope: &Scope,
    debug_ext: bool,
    alloc_ext: bool,
    opt: &OptimizationOptions,
) -> Result<String, Vec<CodeParseError>> {
    compiler_ws::compile_with_full_options(scope, debug_ext, alloc_ext, opt.peephole)
        .map(|prog| prog.to_debug_string())
        .map_err(|e| vec![compile_error_to_code_parse_error(e)])
}

/// Whitespace にコンパイル（従来互換）
#[deprecated(since = "0.2.0", note = "Use compile_to_ws with WsCompileOptions instead")]
#[allow(deprecated)]
pub fn compile_to_whitespace(scope: &Scope) -> Result<String, Vec<CodeParseError>> {
    compile_to_whitespace_with_options(scope, false, false)
}

/// Whitespace にコンパイル（デバッグ用ニーモニック、従来互換）
#[deprecated(since = "0.2.0", note = "Use compile_to_ws with WsCompileOptions instead")]
#[allow(deprecated)]
pub fn compile_to_whitespace_debug(scope: &Scope) -> Result<String, Vec<CodeParseError>> {
    compile_to_whitespace_debug_with_options(scope, false, false)
}

/// `CompileError` を `CodeParseError` に変換する
///
/// WASM API や CLI で既存のエラーハンドリングを再利用するため、
/// `CompileError` を `CodeParseError` 形式に変換する。
fn compile_error_to_code_parse_error(e: compiler_ws::CompileError) -> CodeParseError {
    let code_pointer = e.location.map(|loc| loc.start);
    CodeParseError::new(code_pointer, e.kind.to_string())
}
