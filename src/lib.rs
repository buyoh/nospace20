use std::{collections::BTreeMap, iter, str};

pub enum Token {
    Number(i64),
    Identifier(String),
    Symbol(char),
    ParenthesisL, // (
    ParenthesisR, // )
    BracketL,     // [
    BracketR,     // ]
    BraceL,       // {
    BraceR,       // }
    Semicolon,    // ;
    Colon,        // ;
}

fn parse_number(iter: &mut iter::Peekable<str::Chars>) -> Token {
    let mut value = 0 as i64;
    while let Some(c) = iter.peek() {
        if !c.is_ascii_digit() {
            // TODO: minus
            // TODO: 0x
            break;
        }
        let d = c.to_digit(10).unwrap();
        value = value * 10 + d as i64;
        iter.next();
    }
    Token::Number(value)
}

fn parse_identifier(iter: &mut iter::Peekable<str::Chars>) -> Token {
    if let Some('A'..='Z') | Some('a'..='z') | Some('_') = iter.peek() {
    } else {
        panic!("internal error");
    }
    let mut id = String::new();
    loop {
        if let Some('A'..='Z') | Some('a'..='z') | Some('_') | Some('0'..='9') = iter.peek() {
            id.push(iter.next().unwrap());
        } else {
            id.shrink_to_fit();
            return Token::Identifier(id);
        }
    }
}

pub fn parse_to_tokens(text: &String) -> Vec<Token> {
    let mut tokens = Vec::<Token>::new();
    let mut iter = text.chars().peekable();
    while let Some(c) = iter.peek() {
        if c.is_ascii_digit() {
            tokens.push(parse_number(&mut iter));
        } else if c.is_whitespace() {
            iter.next();
        } else {
            match *c {
                '+' | '-' | '*' | '/' | '=' => {
                    tokens.push(Token::Symbol(*c));
                    iter.next();
                }
                'A'..='Z' | 'a'..='z' | '_' => {
                    tokens.push(parse_identifier(&mut iter));
                }
                '(' => {
                    tokens.push(Token::ParenthesisL);
                    iter.next();
                }
                ')' => {
                    tokens.push(Token::ParenthesisR);
                    iter.next();
                }
                ';' => {
                    tokens.push(Token::Semicolon);
                    iter.next();
                }
                ':' => {
                    tokens.push(Token::Colon);
                    iter.next();
                }
                _ => panic!("invalid char: {}", c),
            }
        }
    }
    tokens
}

//

pub enum Operator2 {
    Plus,
    Minus,
    Multiply,
    Divide,
    Assign,
}

pub enum Expression {
    Operation2(Operator2, Box<Expression>, Box<Expression>),
    Function(String, Vec<Box<Expression>>),
    Factor(i64),
    Variable(String),
}

fn parse_to_expression_tree_function(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    if let Some(Token::Identifier(name)) = iter.next() {
        if let Some(Token::ParenthesisL) = iter.next() {
            // TODO: unary only
            let e = parse_to_expression_tree_root(iter);
            if let Some(Token::ParenthesisR) = iter.next() {
                return Box::new(Expression::Function(name.clone(), vec![e]));
            }
        }
    }
    panic!("syntax error: symbol");
}

fn parse_to_expression_tree_factor(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    if let Some(token) = iter.peek() {
        match token {
            Token::Number(val) => {
                iter.next();
                return Box::new(Expression::Factor(*val));
            }
            Token::Identifier(id) => match id.as_str() {
                "one" => {
                    iter.next();
                    return Box::new(Expression::Factor(1));
                }
                "two" => {
                    iter.next();
                    return Box::new(Expression::Factor(2));
                }
                "pow2" => return parse_to_expression_tree_function(iter),
                _ => {
                    // TODO: confirm whether the identifier is declared
                    // panic!("syntax error: unknown identifier")
                    iter.next();
                    return Box::new(Expression::Variable(id.clone()));
                }
            },
            Token::ParenthesisL => {
                iter.next();
                let e = parse_to_expression_tree_root(iter);
                if let Some(Token::ParenthesisR) = iter.next() {
                    return e;
                }
                panic!("syntax error: expected ')'");
            }
            _ => panic!("syntax error: symbol"),
        }
    }
    panic!("syntax error: terminal");
}

fn parse_to_expression_tree_mul(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    let mut left = parse_to_expression_tree_factor(iter);
    loop {
        let op = if let Some(token) = iter.peek() {
            match token {
                Token::Symbol(chr) => match *chr {
                    '*' => Operator2::Multiply,
                    '/' => Operator2::Divide,
                    _ => return left,
                },
                _ => return left,
            }
        } else {
            return left;
        };
        iter.next();
        let right = parse_to_expression_tree_factor(iter);
        left = Box::new(Expression::Operation2(op, left, right))
    }
}

fn parse_to_expression_tree_plus(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    let mut left = parse_to_expression_tree_mul(iter);
    loop {
        let op = if let Some(token) = iter.peek() {
            match token {
                Token::Symbol(chr) => match *chr {
                    '+' => Operator2::Plus,
                    '-' => Operator2::Minus,
                    _ => return left,
                },
                _ => return left,
            }
        } else {
            return left;
        };
        iter.next();
        let right = parse_to_expression_tree_mul(iter);
        left = Box::new(Expression::Operation2(op, left, right));
    }
}

fn parse_to_expression_tree_assign(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    let left = parse_to_expression_tree_plus(iter);
    let op = if let Some(token) = iter.peek() {
        match token {
            Token::Symbol(chr) => match *chr {
                '=' => Operator2::Assign,
                _ => return left,
            },
            _ => return left,
        }
    } else {
        return left;
    };
    iter.next();
    let right = parse_to_expression_tree_assign(iter);
    Box::new(Expression::Operation2(op, left, right))
}

fn parse_to_expression_tree_root(
    iter: &mut iter::Peekable<std::slice::Iter<Token>>,
) -> Box<Expression> {
    parse_to_expression_tree_assign(iter)
}

pub enum Statement {
    VariableDeclaration(String, Box<Expression>),
    Expression(Box<Expression>),
}

fn parse_to_statements_let(iter: &mut iter::Peekable<std::slice::Iter<Token>>) -> Statement {
    iter.next(); // let
    if let Some(Token::Colon) = iter.next() {
        if let Some(Token::Identifier(id)) = iter.next() {
            if let Some(Token::Semicolon) = iter.next() {
                return Statement::VariableDeclaration(id.clone(), Box::new(Expression::Factor(0)));
            } else {
                panic!("syntax error: expected ';'");
            }
        } else {
            panic!("syntax error: expected ':'");
        }
    } else {
        panic!("syntax error: expected ':'");
    }
}

fn parse_to_statements(iter: &mut iter::Peekable<std::slice::Iter<Token>>) -> Vec<Statement> {
    let mut statements = Vec::<Statement>::new();
    while let Some(token) = iter.peek() {
        if let Token::Identifier(identifier) = token {
            if identifier == "let" {
                statements.push(parse_to_statements_let(iter));
                continue;
            }
        }
        let expr = parse_to_expression_tree_root(iter);
        if let Some(Token::Semicolon) = iter.next() {
            statements.push(Statement::Expression(expr));
        } else {
            panic!("syntax error: expected ';'");
        }
    }
    return statements;
    // panic!("syntax error: terminal");
}

pub fn parse_to_tree(tokens: &Vec<Token>) -> Vec<Statement> {
    let mut iter = tokens.iter().peekable();
    parse_to_statements(&mut iter)
}

//

fn interpret_expression(expr: &Box<Expression>, variables: &mut BTreeMap<String, i64>) -> i64 {
    match expr.as_ref() {
        Expression::Operation2(op, left, right) => match op {
            Operator2::Plus => {
                interpret_expression(left, variables) + interpret_expression(right, variables)
            }
            Operator2::Minus => {
                interpret_expression(left, variables) - interpret_expression(right, variables)
            }
            Operator2::Multiply => {
                interpret_expression(left, variables) * interpret_expression(right, variables)
            }
            Operator2::Divide => {
                interpret_expression(left, variables) / interpret_expression(right, variables)
            }
            Operator2::Assign => {
                if let Expression::Variable(name) = left.as_ref() {
                    if variables.contains_key(name) {
                        // todo: more nice impl
                        // todo: should be checked not in runtime.
                        let v = interpret_expression(right, variables);
                        variables.insert(name.clone(), v);
                        v
                    } else {
                        panic!("syntax error: unknown variable name")
                    }
                } else {
                    panic!("runtime error: left value is not variable");
                }
            }
        },
        Expression::Function(id, args) => {
            if id == "pow2" {
                let a = interpret_expression(args.first().unwrap(), variables);
                a * a
            } else {
                panic!("syntax error: unknown identifier")
            }
        }
        Expression::Factor(v) => *v,
        Expression::Variable(name) => {
            if let Some(val) = variables.get(name) {
                *val
            } else {
                panic!("syntax error: unknown variable name")
            }
        }
    }
}

fn interpret_statement(statement: &Statement, variables: &mut BTreeMap<String, i64>) {
    match statement {
        Statement::VariableDeclaration(name, expr) => {
            if variables.contains_key(name) {
                panic!("runtime error: redeclaration");
            }
            let v = interpret_expression(&expr, variables);
            variables.insert(name.clone(), v);
        }
        Statement::Expression(expr) => {
            let x = interpret_expression(&expr, variables);
            println!("ans = {}", x);
        }
    }
}

pub fn interpret_statements(statements: &Vec<Statement>) {
    let mut variables = BTreeMap::<String, i64>::new();
    for statement in statements {
        interpret_statement(statement, &mut variables);
    }
}
