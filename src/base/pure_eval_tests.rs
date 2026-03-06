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
    assert_eq!(
        eval_unary_pure(&Operator1::Negative, i64::MIN),
        Some(i64::MIN)
    );
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
