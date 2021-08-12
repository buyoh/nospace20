use nospace20::{interpret_statements, parse_to_tokens, parse_to_tree};

fn main() {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok();
    let t = parse_to_tokens(&line);
    let s = parse_to_tree(&t);
    interpret_statements(&s);
}
