//! whitespace20 - A Whitespace language interpreter

use clap::{Parser, ValueEnum};
use nospace20::runner::{build_vm, run_vm_to_end, WsRunError, WsRunOptions};
use nospace20::whitespace::{max_memory_cells_from_setting, DEFAULT_MAX_MEMORY_CELLS};
use nospace20::TargetExtension;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::process;

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

/// whitespace20 - A Whitespace language interpreter
#[derive(Parser, Debug)]
#[command(name = "whitespace20")]
#[command(version)]
#[command(about = "A Whitespace language interpreter")]
#[command(after_help = "\
Pass the same --std-ext to nospace20 and whitespace20 (e.g. `--std-ext debug` for both); \
otherwise __assert / __trace silently have no effect.
Only Whitespace code is accepted (not the mnemonic output of `nospace20 --target mnemonic`).
To compile and run a nospace program at once, use tools/nsrun.sh.")]
struct Args {
    /// Whitespace source file to execute ("-" reads the source from stdin;
    /// then the program's stdin is empty unless --input is given)
    file: String,

    /// Read stdin from file
    #[arg(short, long, value_name = "FILE")]
    input: Option<String>,

    /// Write stdout to file
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Maximum execution steps (0 = unlimited)
    #[arg(short, long, default_value_t = 0)]
    max_steps: usize,

    /// Show execution metrics after run
    #[arg(long)]
    debug: bool,

    /// Treat uninitialized heap access as an error (like wsc default behavior)
    #[arg(long)]
    strict_heap: bool,

    /// Return a deterministic non-zero value for uninitialized heap access (for detecting zero-init dependency bugs)
    #[arg(long)]
    randomize_heap: bool,

    /// Maximum memory used by the VM, in cells: heap addresses + data stack + return addresses (0 = unlimited)
    #[arg(long, default_value_t = DEFAULT_MAX_MEMORY_CELLS)]
    max_memory_cells: usize,

    /// Standard extensions (can be specified multiple times; use the same value as for nospace20)
    #[arg(long = "std-ext", value_enum)]
    std_ext: Vec<CliTargetExt>,
}

/// ソースを読む。`-` なら stdin から読む
fn read_source(file: &str) -> Result<String, String> {
    if file == "-" {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| format!("Error reading source from stdin: {}", e))?;
        Ok(source)
    } else {
        fs::read_to_string(file).map_err(|e| format!("Error reading file '{}': {}", file, e))
    }
}

fn main() {
    let args = Args::parse();

    let target_extensions: Vec<TargetExtension> =
        args.std_ext.into_iter().map(|e| e.into()).collect();
    let debug_ext = target_extensions.contains(&TargetExtension::Debug);

    let source = read_source(&args.file).unwrap_or_else(|msg| {
        eprintln!("{}", msg);
        process::exit(1);
    });

    // VM 初期化
    let options = WsRunOptions {
        debug_ext,
        max_memory_cells: max_memory_cells_from_setting(args.max_memory_cells),
        strict_heap: args.strict_heap,
        randomize_heap: args.randomize_heap,
    };
    let mut vm = build_vm(&source, &options).unwrap_or_else(|e| {
        eprintln!("Parse error: {}", e);
        process::exit(1);
    });

    // stdin 設定（ソースを stdin から読んだ場合は、stdin はすでに終端まで読まれている）
    let stdin: Box<dyn std::io::BufRead> = if let Some(input_file) = &args.input {
        match fs::File::open(input_file) {
            Ok(file) => Box::new(BufReader::new(file)),
            Err(e) => {
                eprintln!("Error opening input file '{}': {}", input_file, e);
                process::exit(1);
            }
        }
    } else {
        Box::new(BufReader::new(std::io::stdin()))
    };

    // stdout 設定
    let stdout: Box<dyn Write> = if let Some(output_file) = &args.output {
        match fs::File::create(output_file) {
            Ok(file) => Box::new(file),
            Err(e) => {
                eprintln!("Error creating output file '{}': {}", output_file, e);
                process::exit(1);
            }
        }
    } else {
        Box::new(std::io::stdout())
    };

    vm = vm.with_io(stdin, stdout);
    if debug_ext {
        // __clog の出力はプログラムの stdout と分けて stderr に書く
        vm = vm.with_clog_writer(Box::new(std::io::stderr()));
    }

    // 実行
    let max_steps = if args.max_steps == 0 {
        usize::MAX
    } else {
        args.max_steps
    };

    // BUG FIX: 従来は `vm.flush()` が flush の失敗を捨てていたため、出力先への書き込みが
    // 失敗しても（`> /dev/full` など）exit 0 になっていた。run_vm_to_end は正常終了時の
    // flush の失敗を WsRunError::Flush として返す
    let result = run_vm_to_end(&mut vm, max_steps);

    // デバッグ情報（失敗時も表示する）
    if args.debug {
        eprintln!("Total steps: {}", vm.total_steps());
        eprintln!("Final stack size: {}", vm.data_stack().len());
        eprintln!("Heap size: {}", vm.heap().len());
        if !vm.traced.is_empty() {
            eprintln!("Traced values: {:?}", vm.traced);
        }
    }

    // 結果に応じて終了コード設定
    match result {
        Ok(()) => process::exit(0),
        Err(WsRunError::Runtime(e)) => eprintln!("Runtime error: {}", e),
        Err(WsRunError::StepLimitExceeded(steps)) => {
            eprintln!("Error: Execution limit exceeded ({} steps)", steps)
        }
        // CLI は interactive stdin を使わないため通常到達しない
        Err(WsRunError::UnexpectedInputWait) => {
            eprintln!("Error: Unexpected input wait in non-interactive mode")
        }
        Err(e) => eprintln!("Error: {}", e),
    }
    process::exit(1);
}
