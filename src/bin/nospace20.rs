use nospace20::{interpret_expression, parse_to_tokens, parse_to_tree};

fn main() {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok();
    let t = parse_to_tokens(&line);
    let e = parse_to_tree(&t);
    println!("ans = {}", interpret_expression(&e));
}
