#[cfg(test)]
#[macro_use]
extern crate assert_matches;

pub use base::error::{CompileStage, NospaceError};
/// `Rc<RefCell<Vec<u8>>>` に書き込む writer（VM の stdout / clog の出力をメモリ上で受け取るのに使う）
pub use base::shared_writer::SharedWriter;
pub use base::CodeParseError;
pub use compiler_ws::CompileError;
pub use logger::TextCode;
pub use semantic_analyzer::{Program, Scope};
use token_parser::PrettyToken;
use tree_parser::LocatedStatement;

pub mod algorithm;
mod base;
mod compile_property;
pub mod compiler_ws;
mod logger;
pub mod optimizer;
pub mod runner;
mod semantic_analyzer;
mod token_parser;
mod tree_parser;
pub mod whitespace;

#[cfg(feature = "cli")]
pub mod cli_utils;

#[cfg(feature = "wasm")]
mod wasm_api;

pub use compile_property::{
    CompileProperty, CompileTarget, LanguageStd, TargetExtension, ValidationError,
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

/// 意味解析を実行し、プログラム（ルートスコープ・全関数など）を構築する
pub fn semantic_analyze(root: &Vec<LocatedStatement>) -> Result<Program, Vec<CodeParseError>> {
    semantic_analyzer::analyze(root)
}

/// Program に対して最適化パスを適用する
pub fn optimize(program: &mut Program, options: &OptimizationOptions) {
    optimizer::optimize(program, options);
}

/// nospace のソースを意味解析まで通し、最適化パスが有効なら適用する
///
/// `parse_to_tokens → parse_to_tree → semantic_analyze → optimize` をまとめたもの。
/// CLI・WASM・テストが共通に使う。
pub fn analyze_source(
    source: &str,
    options: &OptimizationOptions,
) -> Result<Program, Vec<CodeParseError>> {
    let tokens = parse_to_tokens(&source.to_string())?;
    let tree = parse_to_tree(&tokens)?;
    let mut program = semantic_analyze(&tree)?;
    if options.any_enabled() {
        optimize(&mut program, options);
    }
    Ok(program)
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
    program: &Program,
    options: &WsCompileOptions,
) -> Result<String, CompileError> {
    let prog = compiler_ws::compile_with_full_options(
        program,
        options.debug_ext,
        options.alloc_ext,
        options.optimization.peephole,
    )?;

    match options.output_format {
        WsOutputFormat::Whitespace => Ok(prog.to_whitespace()),
        WsOutputFormat::Mnemonic => Ok(prog.to_debug_string()),
    }
}

/// nospace のソースを Whitespace にコンパイルする
///
/// `analyze_source`（`options.optimization` の最適化を含む）→ `compile_to_ws` をまとめたもの。
/// CLI・WASM・テストが共通に使う。エラーは `NospaceError::Parse` / `NospaceError::Compile`
pub fn compile_source(source: &str, options: &WsCompileOptions) -> Result<String, NospaceError> {
    let program = analyze_source(source, &options.optimization).map_err(NospaceError::Parse)?;
    compile_to_ws(&program, options).map_err(NospaceError::Compile)
}
