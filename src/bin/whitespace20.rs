//! whitespace20 - A Whitespace language interpreter

use clap::{Parser, ValueEnum};
use nospace20::whitespace::{max_memory_cells_from_setting, StepResult, WhitespaceVM};
use nospace20::TargetExtension;
use std::fs;
use std::io::{BufReader, Write};
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
struct Args {
    /// Whitespace source file to execute
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
    #[arg(long, default_value_t = 0)]
    max_memory_cells: usize,

    /// Standard extensions (can be specified multiple times, for compatibility with nospace20)
    #[arg(long = "std-ext", value_enum)]
    std_ext: Vec<CliTargetExt>,
}

fn main() {
    let args = Args::parse();

    let target_extensions: Vec<TargetExtension> =
        args.std_ext.into_iter().map(|e| e.into()).collect();
    let debug_ext = target_extensions.contains(&TargetExtension::Debug);

    // ファイル読み込み
    let source = match fs::read_to_string(&args.file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", args.file, e);
            process::exit(1);
        }
    };

    // VM 初期化
    let mut vm = match WhitespaceVM::from_source(&source) {
        Ok(vm) => vm
            .with_debug_ext(debug_ext)
            .with_strict_heap(args.strict_heap)
            .with_randomize_heap(args.randomize_heap)
            .with_max_memory_cells(max_memory_cells_from_setting(args.max_memory_cells)),
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    // stdin 設定
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

    let result = vm.run(max_steps);
    vm.flush();

    // デバッグ情報
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
        StepResult::Complete => {
            process::exit(0);
        }
        StepResult::Suspended => {
            eprintln!("Error: Execution limit exceeded ({} steps)", max_steps);
            process::exit(1);
        }
        StepResult::Error(e) => {
            eprintln!("Runtime error: {}", e);
            process::exit(1);
        }
        StepResult::WaitingForInput(_) => {
            // CLI は interactive stdin を使わないため通常到達しない
            eprintln!("Error: Unexpected input wait in non-interactive mode");
            process::exit(1);
        }
    }
}
