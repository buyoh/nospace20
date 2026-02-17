//! whitespace20 - A Whitespace language interpreter

use clap::Parser;
use nospace20::whitespace::{RuntimeError, StepResult, WhitespaceVM};
use std::fs;
use std::io::{BufReader, Write};
use std::process;

/// whitespace20 - A Whitespace language interpreter
#[derive(Parser, Debug)]
#[command(name = "whitespace20")]
#[command(version = "0.1.0")]
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
}

fn main() {
    let args = Args::parse();

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
        Ok(vm) => vm,
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
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
            let error_msg = match e {
                RuntimeError::StackUnderflow => "Stack underflow",
                RuntimeError::DivisionByZero => "Division by zero",
                RuntimeError::UndefinedLabel(id) => {
                    eprintln!("Undefined label: {}", id);
                    process::exit(1);
                }
                RuntimeError::UninitializedHeap(addr) => {
                    eprintln!("Uninitialized heap access at address: {}", addr);
                    process::exit(1);
                }
                RuntimeError::CallStackUnderflow => "Call stack underflow",
                RuntimeError::ProgramCounterOutOfBounds => "Program counter out of bounds",
                RuntimeError::IoError(msg) => {
                    eprintln!("I/O error: {}", msg);
                    process::exit(1);
                }
                RuntimeError::AssertionFailed(val) => {
                    eprintln!("Assertion failed with value: {}", val);
                    process::exit(1);
                }
            };
            eprintln!("Runtime error: {}", error_msg);
            process::exit(1);
        }
    }
}
