use std::{
    io::{Read, Write},
    iter::repeat,
    process,
};

use clap::{Parser, ValueEnum};
use nospace20::cli_utils::CliCompileArgs;
use nospace20::{
    compile_source, CodeParseError, CompileProperty, CompileTarget, NospaceError, TargetExtension,
    TextCode, ValidationError, WsCompileOptions,
};
use unicode_width::UnicodeWidthStr;

/// コンパイルターゲット
///
/// `Json` は未対応だが、`--target json` を「未実装」のエラーとして伝えるために受け付ける
/// （ライブラリの `CompileTarget` には無い。変換で拒否する）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum CliTarget {
    #[default]
    Ws,
    Mnemonic,
    Json,
}

impl TryFrom<CliTarget> for CompileTarget {
    type Error = ValidationError;

    fn try_from(cli: CliTarget) -> Result<Self, Self::Error> {
        match cli {
            CliTarget::Ws => Ok(CompileTarget::Ws),
            CliTarget::Mnemonic => Ok(CompileTarget::Mnemonic),
            CliTarget::Json => Err(ValidationError::UnimplementedFeature(
                "--target=json".to_string(),
            )),
        }
    }
}

/// nospace - A nospace language compiler (compiles to Whitespace; run it with whitespace20)
#[derive(Parser, Debug)]
#[command(name = "nospace20")]
#[command(version)]
#[command(
    about = "A nospace language compiler (compiles to Whitespace; run it with whitespace20)",
    long_about = None
)]
#[command(after_help = "\
To run the output, pass the same --std-ext to whitespace20, e.g.
  nospace20 --std-ext debug a.ns -o a.ws && whitespace20 --std-ext debug a.ws
To compile and run at once, use tools/nsrun.sh.")]
struct Args {
    /// Source file to compile (reads from stdin if not provided)
    file: Option<String>,

    #[command(flatten)]
    compile: CliCompileArgs,

    /// Compile target (default: ws)
    #[arg(long, value_enum)]
    target: Option<CliTarget>,

    /// Output file (stdout if not specified)
    #[arg(short, long)]
    output: Option<String>,

    /// Print the output file name to stderr after compiling
    #[arg(short, long)]
    debug: bool,
}

// BUG FIX: エラーを `println!`（stdout）で出していたため、`nospace20 x.ns > out.ws` の
// ように compile 結果をリダイレクトすると、エラー文言が出力ファイルに混入していた。
// エラーは stderr に出すことで、正常出力（stdout）と分離する。
fn report_parse_errors(errors: &[CodeParseError], text: &TextCode) -> ! {
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

fn main() {
    let args = Args::parse();

    // Build optimization options first
    let opt_options = args.compile.build_optimization_options();

    // Validation: 未対応のターゲット（--target json）はここで拒否する
    let target = match args.target.map(CompileTarget::try_from).transpose() {
        Ok(target) => target,
        Err(err) => {
            eprintln!("error: {}", err);
            process::exit(1);
        }
    };

    // Build CompileProperty
    let property = CompileProperty {
        std: args.compile.std.into(),
        target,
        target_extensions: args.compile.build_target_extensions(),
        output: args.output,
        debug: args.debug,
    };

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
    // コンパイル
    let debug_ext = property.target_extensions.contains(&TargetExtension::Debug);
    let alloc_ext = property.target_extensions.contains(&TargetExtension::Alloc);
    let ws_options = WsCompileOptions {
        debug_ext,
        alloc_ext,
        output_format: property.target_or_default().output_format(),
        optimization: opt_options,
    };
    let output = match compile_source(&code_raw, &ws_options) {
        Ok(s) => s,
        Err(NospaceError::Parse(errors)) => report_parse_errors(&errors, &text),
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
        // BUG FIX: `print!` は書き込みの失敗（`| head` で閉じられた、`> /dev/full` など）で panic し、
        // flush の失敗は検出しなかった。コンパイルが主な用途になったため、失敗をエラーとして報告する
        let mut stdout = std::io::stdout();
        if let Err(err) = stdout
            .write_all(output.as_bytes())
            .and_then(|()| stdout.flush())
        {
            eprintln!("error: failed to write to stdout: {}", err);
            process::exit(1);
        }
    }
}
