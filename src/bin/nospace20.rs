use std::{io::Read, iter::repeat, process};

use nospace20::{
    interpret_func, parse_to_tokens, parse_to_tree, syntactic_analyze, CodeParseError, TextCode,
};
use unicode_width::UnicodeWidthStr;

fn handle_parse_error<T>(res: Result<T, Vec<CodeParseError>>, text: &TextCode) -> T {
    let errors = match res {
        Ok(x) => return x,
        Err(e) => e,
    };

    for error in errors.iter().take(3) {
        println!("error: {}", error.message);
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
    let mut code_raw = String::new();
    std::io::stdin().read_to_string(&mut code_raw).ok();
    let text = TextCode::new(&code_raw);
    let t = handle_parse_error(parse_to_tokens(&code_raw), &text);
    let s = handle_parse_error(parse_to_tree(&t), &text);
    let a = syntactic_analyze(&s);
    let result = interpret_func(&a, "main");
    if let Some(val) = result {
        println!("main returns: {}", val);
    } else {
        println!("main exited");
    }
}
