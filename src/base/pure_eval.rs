use crate::tree_parser::{Operator1, Operator2};

/// bool を nospace の整数表現（0/1）に変換する
pub(crate) fn bool_to_int(b: bool) -> i64 {
    if b { 1 } else { 0 }
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
mod tests {
    use super::*;
    use crate::tree_parser::{Operator1, Operator2};

    #[test]
    fn test_eval_binary_pure_arithmetic() {
        assert_eq!(eval_binary_pure(&Operator2::Plus, 3, 4), Some(7));
        assert_eq!(eval_binary_pure(&Operator2::Minus, 10, 3), Some(7));
        assert_eq!(eval_binary_pure(&Operator2::Multiply, 3, 4), Some(12));
        assert_eq!(eval_binary_pure(&Operator2::Divide, 10, 2), Some(5));
        assert_eq!(eval_binary_pure(&Operator2::Modulo, 10, 3), Some(1));
    }

    #[test]
    fn test_eval_binary_pure_zero_division() {
        assert_eq!(eval_binary_pure(&Operator2::Divide, 10, 0), None);
        assert_eq!(eval_binary_pure(&Operator2::Modulo, 10, 0), None);
    }

    #[test]
    fn test_eval_binary_pure_comparison() {
        assert_eq!(eval_binary_pure(&Operator2::Equal, 5, 5), Some(1));
        assert_eq!(eval_binary_pure(&Operator2::Equal, 5, 6), Some(0));
        assert_eq!(eval_binary_pure(&Operator2::NotEqual, 5, 6), Some(1));
        assert_eq!(eval_binary_pure(&Operator2::Less, 3, 5), Some(1));
        assert_eq!(eval_binary_pure(&Operator2::Less, 5, 3), Some(0));
        assert_eq!(eval_binary_pure(&Operator2::LessEqual, 5, 5), Some(1));
        assert_eq!(eval_binary_pure(&Operator2::Greater, 5, 3), Some(1));
        assert_eq!(eval_binary_pure(&Operator2::GreaterEqual, 5, 5), Some(1));
    }

    #[test]
    fn test_eval_binary_pure_non_pure_ops() {
        assert_eq!(eval_binary_pure(&Operator2::Assign, 1, 2), None);
        assert_eq!(eval_binary_pure(&Operator2::LogicalAnd, 1, 1), None);
        assert_eq!(eval_binary_pure(&Operator2::LogicalOr, 0, 1), None);
    }

    #[test]
    fn test_eval_binary_pure_wrapping() {
        // i64::MAX + 1 はオーバーフローせず wrapping で処理
        assert_eq!(
            eval_binary_pure(&Operator2::Plus, i64::MAX, 1),
            Some(i64::MIN)
        );
        // i64::MIN * -1 はオーバーフローせず wrapping で処理
        assert_eq!(
            eval_binary_pure(&Operator2::Multiply, i64::MIN, -1),
            Some(i64::MIN)
        );
    }

    #[test]
    fn test_eval_unary_pure_negative() {
        assert_eq!(eval_unary_pure(&Operator1::Negative, 5), Some(-5));
        assert_eq!(eval_unary_pure(&Operator1::Negative, -5), Some(5));
        // wrapping: i64::MIN の符号反転
        assert_eq!(eval_unary_pure(&Operator1::Negative, i64::MIN), Some(i64::MIN));
    }

    #[test]
    fn test_eval_unary_pure_logical_not() {
        assert_eq!(eval_unary_pure(&Operator1::LogicalNot, 0), Some(1));
        assert_eq!(eval_unary_pure(&Operator1::LogicalNot, 1), Some(0));
        assert_eq!(eval_unary_pure(&Operator1::LogicalNot, -5), Some(0));
    }

    #[test]
    fn test_eval_unary_pure_non_pure_ops() {
        assert_eq!(eval_unary_pure(&Operator1::Ref, 5), None);
        assert_eq!(eval_unary_pure(&Operator1::Deref, 42), None);
    }

    #[test]
    fn test_bool_to_int() {
        assert_eq!(bool_to_int(true), 1);
        assert_eq!(bool_to_int(false), 0);
    }
}
