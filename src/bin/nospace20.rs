use std::io::Read;

use nospace20::{interpret_main_func, parse_to_tokens, parse_to_tree, syntactic_analyze};

fn main() {
    let mut line = String::new();
    std::io::stdin().read_to_string(&mut line).ok();
    let t = parse_to_tokens(&line);
    let s = parse_to_tree(&t);
    let a = syntactic_analyze(&s);
    interpret_main_func(&a);
}
