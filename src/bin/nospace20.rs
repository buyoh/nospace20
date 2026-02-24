use std::{io::Read, iter::repeat, process};

use clap::{Parser, ValueEnum};
use nospace20::{
    compile_to_whitespace_debug_with_options,
    compile_to_whitespace_with_options,
    interpret_with_env,
    parse_to_tokens,
    parse_to_tree,
    syntactic_analyze, // 後方互換性のためのエイリアス (実体は semantic_analyzer::analyze)
    CodeParseError,
    CompileProperty,
    CompileTarget,
    Environment,
    ExecutionMode,
    LanguageStd,
    TargetExtension,
    TextCode,
};
use unicode_width::UnicodeWidthStr;

/// 言語サブセット
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum CliStd {
    #[default]
    Standard,
    Min,
    Ws,
}

impl From<CliStd> for LanguageStd {
    fn from(cli: CliStd) -> Self {
        match cli {
            CliStd::Standard => LanguageStd::Standard,
            CliStd::Min => LanguageStd::Min,
            CliStd::Ws => LanguageStd::Ws,
        }
    }
}

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

/// ターゲット拡張
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliTargetExt {
    Debug,
}

impl From<CliTargetExt> for TargetExtension {
    fn from(cli: CliTargetExt) -> Self {
        match cli {
            CliTargetExt::Debug => TargetExtension::Debug,
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

    /// Language subset
    #[arg(long, value_enum, default_value_t = CliStd::Standard)]
    std: CliStd,

    /// Execution mode
    #[arg(long, value_enum, default_value_t = CliMode::Run)]
    mode: CliMode,

    /// Compile target (only with --mode=compile)
    #[arg(long, value_enum, default_value_t = CliTarget::Ws)]
    target: CliTarget,

    /// Standard extensions (only with --mode=compile, can be specified multiple times)
    #[arg(long = "std-ext", value_enum)]
    std_ext: Vec<CliTargetExt>,

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

    for error in errors.iter().take(3) {
        println!("error: {}", error.message);

        // デバッグビルド時は内部位置情報を表示
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
            println!("line:{} column:{}", line_no, column);
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

    process::exit(1);
}

fn main() {
    let args = Args::parse();

    // CompileProperty を構築
    let property = CompileProperty {
        std: args.std.into(),
        mode: args.mode.into(),
        target: args.target.into(),
        target_extensions: args.std_ext.into_iter().map(|e| e.into()).collect(),
        output: args.output,
        debug: args.debug,
        ignore_debug: args.ignore_debug,
    };

    // バリデーション
    if let Err(err) = property.validate() {
        eprintln!("error: {}", err);
        process::exit(1);
    }

    // ソースコードの読み込み
    let code_raw = if let Some(file_path) = args.file {
        // ファイルから読み込み
        match std::fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(err) => {
                eprintln!("error: failed to read file '{}': {}", file_path, err);
                process::exit(1);
            }
        }
    } else {
        // 標準入力から読み込み
        let mut code_raw = String::new();
        std::io::stdin().read_to_string(&mut code_raw).ok();
        code_raw
    };

    let text = TextCode::new(&code_raw);
    let t = handle_parse_error(parse_to_tokens(&code_raw), &text);
    let s = handle_parse_error(parse_to_tree(&t), &text);
    let a = handle_parse_error(syntactic_analyze(&s), &text);

    // モードに応じて処理
    match property.mode {
        ExecutionMode::Run => {
            // main 関数の存在チェック
            if !a.has_function("main") {
                eprintln!("error: function 'main' not found");
                process::exit(1);
            }

            // インタプリタモード
            let config = nospace20::EnvironmentConfig {
                ignore_debug: property.ignore_debug,
                ..Default::default()
            };
            let mut env = Environment::new_with_config(
                Box::new(std::io::BufReader::new(std::io::stdin())),
                Box::new(std::io::stdout()),
                config,
            );
            let result = interpret_with_env(&mut env, &a);

            if let Some(val) = result {
                println!("main returns: {}", val);
            } else {
                println!("main exited");
            }

            // デバッグフラグが有効なら、trace結果を表示
            if property.debug && !env.traced.is_empty() {
                println!("\n=== Trace Results ===");
                for (key, value) in &env.traced {
                    println!("trace[{}]: {}", key, value);
                }
            }
        }
        ExecutionMode::Compile => {
            // コンパイルモード
            let debug_ext = property.target_extensions.contains(&TargetExtension::Debug);
            let compiled = match property.target {
                CompileTarget::Ws => compile_to_whitespace_with_options(&a, debug_ext),
                CompileTarget::Mnemonic => compile_to_whitespace_debug_with_options(&a, debug_ext),
                _ => unreachable!("Unsupported target should be caught by validation"),
            };

            let output = match compiled {
                Ok(code) => code,
                Err(err) => {
                    eprintln!("compilation error: {}", err);
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
