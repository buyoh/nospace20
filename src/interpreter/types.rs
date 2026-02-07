// Block(Vec<Statement>) の評価結果
#[derive(Debug)]
pub(super) enum Flow {
    Proceed,
    Return(i64),
    Continue,
    Break,
}

// Expression の評価結果
pub(super) enum ExpressionFlow {
    Value(i64),
    Jump(Flow),
}

macro_rules! try_expr {
    ($e: expr) => {
        match $e {
            ExpressionFlow::Value(x) => x,
            ExpressionFlow::Jump(f) => return ExpressionFlow::Jump(f),
        }
    };
}

pub(super) use try_expr;

pub(super) fn bool_to_int(x: bool) -> i64 {
    if x {
        1
    } else {
        0
    }
}
