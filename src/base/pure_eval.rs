use crate::tree_parser::{Operator1, Operator2};

/// bool を nospace の整数表現（0/1）に変換する
pub(crate) fn bool_to_int(b: bool) -> i64 {
    if b {
        1
    } else {
        0
    }
}

/// 純粋な二項演算を評価する
///
/// 副作用を持つ演算（Assign 系）や短絡評価が必要な演算（LogicalAnd/Or）は
/// None を返す。0除算も None を返す。
/// オーバーフローは wrapping 演算で処理する。
pub fn eval_binary_pure(op: &Operator2, lhs: i64, rhs: i64) -> Option<i64> {
    match op {
        Operator2::Plus => Some(lhs.wrapping_add(rhs)),
        Operator2::Minus => Some(lhs.wrapping_sub(rhs)),
        Operator2::Multiply => Some(lhs.wrapping_mul(rhs)),
        Operator2::Divide => {
            if rhs != 0 {
                Some(lhs.wrapping_div(rhs))
            } else {
                None
            }
        }
        Operator2::Modulo => {
            if rhs != 0 {
                Some(lhs.wrapping_rem(rhs))
            } else {
                None
            }
        }
        Operator2::Equal => Some(bool_to_int(lhs == rhs)),
        Operator2::NotEqual => Some(bool_to_int(lhs != rhs)),
        Operator2::Less => Some(bool_to_int(lhs < rhs)),
        Operator2::LessEqual => Some(bool_to_int(lhs <= rhs)),
        Operator2::Greater => Some(bool_to_int(lhs > rhs)),
        Operator2::GreaterEqual => Some(bool_to_int(lhs >= rhs)),
        // Assign 系、LogicalAnd/Or は呼び出し元が個別に処理
        _ => None,
    }
}

/// 純粋な単項演算を評価する
///
/// Ref / Deref はランタイム操作のため None を返す。
/// オーバーフローは wrapping 演算で処理する。
pub fn eval_unary_pure(op: &Operator1, val: i64) -> Option<i64> {
    match op {
        Operator1::Negative => Some(val.wrapping_neg()),
        Operator1::LogicalNot => Some(bool_to_int(val == 0)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "pure_eval_tests.rs"]
mod tests;
