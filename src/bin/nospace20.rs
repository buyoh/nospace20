use std::{io::Read, iter::repeat, process};

use nospace20::{
    interpret_main_func, parse_to_tokens, parse_to_tree, syntactic_analyze, CodeParseError,
    TextCode,
};
use unicode_width::UnicodeWidthStr;

fn handle_parse_error<T>(res: Result<T, Vec<CodeParseError>>, text: &TextCode) -> T {
    let errors = match res {
        Ok(x) => return x,
        Err(e) => e,
    };

    for (no, error) in errors.iter().enumerate() {
        if no >= 3 {
            break;
        }
        let code_pointer = error.code_pointer;
        let (line_no, column) = text.char_index_to_line(code_pointer);
        let line_str = text.line(line_no);
        // TODO: no work for non-ascii character.
        // TODO: cover end of input
        println!("error: {}", error.message);
        println!("line:{} column:{}", line_no, column);
        println!("{}", line_str);
        println!(
            "{}^",
            repeat(' ')
                .take(UnicodeWidthStr::width(&line_str[..column]))
                .collect::<String>()
        );
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
    interpret_main_func(&a);
}
