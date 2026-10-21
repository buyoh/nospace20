use std::{
    io::{Read, Write},
    iter::repeat,
    process,
};

use clap::{Parser, ValueEnum};
use nospace20::cli_utils::CliCompileArgs;
use nospace20::whitespace::{StepResult, WhitespaceVM, DEFAULT_MAX_MEMORY_CELLS};
use nospace20::{
    compile_to_ws, optimize, parse_to_tokens, parse_to_tree, semantic_analyze, CodeParseError,
    CompileProperty, CompileTarget, ExecutionMode, OptimizationOptions, Scope, TargetExtension,
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

/// nospace - A nospace language compiler (run mode executes the compiled Whitespace on the built-in VM)
#[derive(Parser, Debug)]
#[command(name = "nospace20")]
#[command(version)]
#[command(
    about = "A nospace language compiler (run mode executes the compiled Whitespace on the built-in VM)",
    long_about = None
)]
struct Args {
    /// Source file to execute (reads from stdin if not provided)
    file: Option<String>,

    #[command(flatten)]
    compile: CliCompileArgs,

    /// Execution mode (run: compile to Whitespace and run it on the built-in VM)
    #[arg(long, value_enum, default_value_t = CliMode::Run)]
    mode: CliMode,

    /// Compile target (only with --mode=compile, default: ws)
    #[arg(long, value_enum)]
    target: Option<CliTarget>,

    /// Output file (only with --mode=compile, stdout if not specified)
    #[arg(short, long)]
    output: Option<String>,

    /// Show trace results after execution
    #[arg(short, long)]
    debug: bool,

    /// Ignore debug built-in functions (__assert, __assert_not, __trace, __clog).
    /// Run mode only: the debug extension is enabled unless this is given
    /// (takes precedence over --std-ext debug there)
    #[arg(long)]
    ignore_debug: bool,
}

// BUG FIX: エラーを `println!`（stdout）で出していたため、`--mode compile > out.ws` の
// ように compile 結果をリダイレクトすると、エラー文言が出力ファイルに混入していた。
// エラーは stderr に出すことで、正常出力（stdout）と分離する。
fn handle_parse_error<T>(res: Result<T, Vec<CodeParseError>>, text: &TextCode) -> T {
    let errors = match res {
        Ok(x) => return x,
        Err(e) => e,
    };

    let total = errors.len();
    for error in errors.iter().take(3) {
        eprintln!("error: {}", error.message);

        // Display internal location information in debug builds
        #[cfg(debug_assertions)]
        {
            eprintln!(
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
            eprintln!("line:{} column:{}", line_no + 1, column + 1);
            eprintln!("{}", line_str);
            eprintln!(
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
        eprintln!("... and {} more errors", total - 3);
    }

    process::exit(1);
}

/// `--mode run`: Whitespace にコンパイルし、内蔵の WhitespaceVM で実行する
///
/// `compile` モードと同じ WS の文字列を経由する（`whitespace20` で実行するのと同じ経路）。
/// debug 拡張は `--ignore-debug` がなければ常に有効（`CompileProperty::run_compile_options`）。
/// エラーは `error: ` を付けずに文言だけを返す。
fn run_on_ws_vm(
    scope: &Scope,
    property: &CompileProperty,
    opt_options: OptimizationOptions,
) -> Result<(), String> {
    let ws_options = property.run_compile_options(opt_options);
    let ws = compile_to_ws(scope, &ws_options).map_err(|e| e.to_string())?;
    let vm = WhitespaceVM::from_source(&ws)
        .map_err(|e| format!("internal error: failed to parse compiled Whitespace: {}", e))?;
    let mut vm = vm
        .with_debug_ext(ws_options.debug_ext)
        .with_max_memory_cells(Some(DEFAULT_MAX_MEMORY_CELLS))
        // __clog の出力はプログラムの stdout と分けて stderr に書く
        .with_clog_writer(Box::new(std::io::stderr()))
        .with_io(
            Box::new(std::io::BufReader::new(std::io::stdin())),
            Box::new(std::io::stdout()),
        );

    let result = vm.run(usize::MAX);
    // エラーの有無にかかわらず、それまでの出力を失わないよう flush する
    vm.flush();
    match result {
        // BUG FIX: WhitespaceVM::flush は flush の失敗を捨てるため、出力先への書き込みが
        // 失敗しても（`> /dev/full` など）exit 0 になっていた。正常終了時は stdout を
        // 直接 flush し、失敗を I/O エラーとして返す
        StepResult::Complete => std::io::stdout()
            .flush()
            .map_err(|e| format!("I/O error: {}", e))?,
        StepResult::Error(err) => return Err(err.to_string()),
        StepResult::Suspended => return Err("execution step limit exceeded".to_string()),
        StepResult::WaitingForInput(_) => return Err("unexpected input wait".to_string()),
    }

    // デバッグフラグが有効なら、trace結果を表示
    // 出力先が閉じられている場合（`| head` など）も panic せずエラーとして返す
    if property.debug && !vm.traced.is_empty() {
        let io_error = |e: std::io::Error| format!("I/O error: {}", e);
        let mut stdout = std::io::stdout();
        writeln!(stdout, "\n=== Trace Results ===").map_err(io_error)?;
        for (key, value) in &vm.traced {
            writeln!(stdout, "trace[{}]: {}", key, value).map_err(io_error)?;
        }
        stdout.flush().map_err(io_error)?;
    }
    Ok(())
}

fn main() {
    let args = Args::parse();

    // Build optimization options first
    let opt_options = args.compile.build_optimization_options();

    // Build CompileProperty
    let property = CompileProperty {
        std: args.compile.std.into(),
        mode: args.mode.into(),
        target: args.target.map(Into::into),
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
        // BUG FIX: 読み取りエラー（不正な UTF-8 など）を `.ok()` で握りつぶしており、
        // 空のソースとして処理が続行されていた。エラーを報告して終了する。
        let mut code_raw = String::new();
        if let Err(err) = std::io::stdin().read_to_string(&mut code_raw) {
            eprintln!("error: failed to read stdin: {}", err);
            process::exit(1);
        }
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

            if let Err(err) = run_on_ws_vm(&a, &property, opt_options) {
                eprintln!("error: {}", err);
                process::exit(1);
            }
        }
        ExecutionMode::Compile => {
            // コンパイルモード
            let debug_ext = property.target_extensions.contains(&TargetExtension::Debug);
            let alloc_ext = property.target_extensions.contains(&TargetExtension::Alloc);
            let output_format = match property.target_or_default() {
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
