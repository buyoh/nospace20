use std::{io::Read, iter::repeat, process};

use clap::Parser;
use nospace20::{
    interpret_func_with_env,
    parse_to_tokens,
    parse_to_tree,
    syntactic_analyze, // 後方互換性のためのエイリアス (実体は semantic_analyzer::analyze)
    CodeParseError,
    Environment,
    TextCode,
};
use unicode_width::UnicodeWidthStr;

/// nospace - A nospace language interpreter
#[derive(Parser, Debug)]
#[command(name = "nospace20")]
#[command(version = "0.1.0")]
#[command(about = "A nospace language interpreter", long_about = None)]
struct Args {
    /// Source file to execute (reads from stdin if not provided)
    file: Option<String>,

    /// Show trace results after execution
    #[arg(short, long)]
    debug: bool,
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
            println!("  (internal: {}:{})", error.caller.file(), error.caller.line());
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

    // Environmentを作成して実行
    let mut env = Environment::new();
    let result = interpret_func_with_env(&mut env, &a, "main");

    if let Some(val) = result {
        println!("main returns: {}", val);
    } else {
        println!("main exited");
    }

    // デバッグフラグが有効なら、trace結果を表示
    if args.debug && !env.traced.is_empty() {
        println!("\n=== Trace Results ===");
        for (key, value) in &env.traced {
            println!("trace[{}]: {}", key, value);
        }
    }
}
