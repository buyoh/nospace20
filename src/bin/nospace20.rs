use std::{
    io::{Read, Write},
    iter::repeat,
    process,
};

use clap::{Parser, ValueEnum};
use nospace20::cli_utils::CliCompileArgs;
use nospace20::{
    compile_to_ws, optimize, parse_to_tokens, parse_to_tree, semantic_analyze, CodeParseError,
    CompileProperty, CompileTarget, ExecutionMode, NospaceVM, Scope, StepResult, TargetExtension,
    TextCode, WsCompileOptions, WsOutputFormat,
};
use unicode_width::UnicodeWidthStr;

/// 実行モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum CliMode {
    #[default]
    Run,
    Compile,
}

impl From<CliMode> for ExecutionMode {
    fn from(cli: CliMode) -> Self {
        match cli {
            CliMode::Run => ExecutionMode::Run,
            CliMode::Compile => ExecutionMode::Compile,
        }
    }
}

/// コンパイルターゲット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum CliTarget {
    #[default]
    Ws,
    Mnemonic,
    Json,
}

impl From<CliTarget> for CompileTarget {
    fn from(cli: CliTarget) -> Self {
        match cli {
            CliTarget::Ws => CompileTarget::Ws,
            CliTarget::Mnemonic => CompileTarget::Mnemonic,
            CliTarget::Json => CompileTarget::Json,
        }
    }
}

/// nospace - A nospace language interpreter and compiler
#[derive(Parser, Debug)]
#[command(name = "nospace20")]
#[command(version = "0.1.0")]
#[command(about = "A nospace language interpreter and compiler", long_about = None)]
struct Args {
    /// Source file to execute (reads from stdin if not provided)
    file: Option<String>,

    #[command(flatten)]
    compile: CliCompileArgs,

    /// Execution mode
    #[arg(long, value_enum, default_value_t = CliMode::Run)]
    mode: CliMode,

    /// Compile target (only with --mode=compile)
    #[arg(long, value_enum, default_value_t = CliTarget::Ws)]
    target: CliTarget,

    /// Output file (only with --mode=compile, stdout if not specified)
    #[arg(short, long)]
    output: Option<String>,

    /// Show trace results after execution
    #[arg(short, long)]
    debug: bool,

    /// Ignore debug built-in functions (__assert, __assert_not, __trace, __clog)
    #[arg(long)]
    ignore_debug: bool,
}

fn handle_parse_error<T>(res: Result<T, Vec<CodeParseError>>, text: &TextCode) -> T {
    let errors = match res {
        Ok(x) => return x,
        Err(e) => e,
    };

    let total = errors.len();
    for error in errors.iter().take(3) {
        println!("error: {}", error.message);

        // Display internal location information in debug builds
        #[cfg(debug_assertions)]
        {
            println!(
                "  (internal: {}:{})",
                error.caller.file(),
                error.caller.line()
            );
        }

        if let Some(code_pointer) = error.code_pointer {
            let (line_no, column) = text.char_index_to_line(code_pointer);
            let line_str = text.line(line_no);
            // NOTE: line_no and column are 0-indexed internally.
            // Convert to 1-indexed for user-facing display.
            println!("line:{} column:{}", line_no + 1, column + 1);
            println!("{}", line_str);
            println!(
                "{}^",
                repeat(' ')
                    .take(UnicodeWidthStr::width(
                        line_str.chars().take(column).collect::<String>().as_str()
                    ))
                    .collect::<String>()
            );
        }
    }

    if total > 3 {
        println!("... and {} more errors", total - 3);
    }

    process::exit(1);
}

/// NospaceVM の 1 回の `step()` で実行する式評価数
///
/// 完了まで繰り返し呼ぶため値は実行結果に影響しない（中断・再開のオーバーヘッドとの兼ね合い）。
const RUN_STEP_BUDGET: usize = 1_000_000;

/// `--mode run`: NospaceVM（明示的スタックマシン）で `__main` を実行する
///
/// 深い再帰でも Rust のスタックを消費せず、実行時エラーは戻り値として受け取る。
fn run_with_vm(scope: Scope, property: &CompileProperty) -> Result<(), nospace20::InterpretError> {
    let config = nospace20::EnvironmentConfig {
        ignore_debug: property.ignore_debug,
        ..Default::default()
    };
    let mut vm = NospaceVM::from_scope(scope)?.with_config(config).with_io(
        Box::new(std::io::BufReader::new(std::io::stdin())),
        Box::new(std::io::stdout()),
    );

    let result = loop {
        match vm.step(RUN_STEP_BUDGET) {
            StepResult::Suspended => continue,
            other => break other,
        }
    };

    let return_value = match result {
        StepResult::Complete { return_value } => return_value,
        StepResult::Error(err) => {
            // 実行時エラーを優先して報告する（出力済み分の flush 失敗は無視）
            let _ = vm.flush();
            return Err(err);
        }
        StepResult::Suspended => unreachable!("loop exits only on non-suspended results"),
    };
    vm.flush()?;

    // 出力先が閉じられている場合（`| head` など）も panic せずエラーとして返す
    let io_error = |e: std::io::Error| nospace20::InterpretError::Io(e.to_string());
    let mut stdout = std::io::stdout();
    match return_value {
        Some(val) => writeln!(stdout, "main returns: {}", val).map_err(io_error)?,
        None => writeln!(stdout, "main exited").map_err(io_error)?,
    }

    // デバッグフラグが有効なら、trace結果を表示
    if property.debug && !vm.traced().is_empty() {
        writeln!(stdout, "\n=== Trace Results ===").map_err(io_error)?;
        for (key, value) in vm.traced() {
            writeln!(stdout, "trace[{}]: {}", key, value).map_err(io_error)?;
        }
    }
    stdout.flush().map_err(io_error)
}

fn main() {
    let args = Args::parse();

    // Build optimization options first
    let opt_options = args.compile.build_optimization_options();

    // Build CompileProperty
    let property = CompileProperty {
        std: args.compile.std.into(),
        mode: args.mode.into(),
        target: args.target.into(),
        target_extensions: args.compile.build_target_extensions(),
        output: args.output,
        debug: args.debug,
        ignore_debug: args.ignore_debug,
    };

    // Validation
    if let Err(err) = property.validate() {
        eprintln!("error: {}", err);
        process::exit(1);
    }

    // Load source code
    let code_raw = if let Some(file_path) = args.file {
        // Read from file
        match std::fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(err) => {
                eprintln!("error: failed to read file '{}': {}", file_path, err);
                process::exit(1);
            }
        }
    } else {
        // Read from stdin
        let mut code_raw = String::new();
        std::io::stdin().read_to_string(&mut code_raw).ok();
        code_raw
    };

    let text = TextCode::new(&code_raw);
    let t = handle_parse_error(parse_to_tokens(&code_raw), &text);
    let s = handle_parse_error(parse_to_tree(&t), &text);
    let mut a = handle_parse_error(semantic_analyze(&s), &text);

    // Apply optimization passes
    if opt_options.any_enabled() {
        optimize(&mut a, &opt_options);
    }

    // Process according to mode
    match property.mode {
        ExecutionMode::Run => {
            // Check for existence of __main function
            if !a.has_function("__main") {
                eprintln!("error: function '__main' not found");
                process::exit(1);
            }

            // Interpreter mode
            if let Err(err) = run_with_vm(a, &property) {
                eprintln!("error: {}", err);
                process::exit(1);
            }
        }
        ExecutionMode::Compile => {
            // コンパイルモード
            let debug_ext = property.target_extensions.contains(&TargetExtension::Debug);
            let alloc_ext = property.target_extensions.contains(&TargetExtension::Alloc);
            let output_format = match property.target {
                CompileTarget::Ws => WsOutputFormat::Whitespace,
                CompileTarget::Mnemonic => WsOutputFormat::Mnemonic,
                _ => unreachable!("Unsupported target should be caught by validation"),
            };
            let ws_options = WsCompileOptions {
                debug_ext,
                alloc_ext,
                output_format,
                optimization: opt_options.clone(),
            };
            let output = match compile_to_ws(&a, &ws_options) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("error: {}", err);
                    process::exit(1);
                }
            };

            // 出力
            if let Some(output_file) = &property.output {
                // ファイルに出力
                if let Err(err) = std::fs::write(output_file, &output) {
                    eprintln!("error: failed to write to '{}': {}", output_file, err);
                    process::exit(1);
                }
                if property.debug {
                    eprintln!("Compiled to: {}", output_file);
                }
            } else {
                // 標準出力に出力
                print!("{}", output);
            }
        }
    }
}
