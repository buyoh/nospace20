use super::*;

#[test]
fn test_total_from_user_size_normal() {
    assert_eq!(total_from_user_size(3), 4); // 3 + 1 = 4
    assert_eq!(total_from_user_size(7), 8); // 7 + 1 = 8
    assert_eq!(total_from_user_size(31), 32); // 31 + 1 = 32
}

#[test]
fn test_total_from_user_size_minimum() {
    // user_size=0 → total=1 < 2 → clamp to 2
    assert_eq!(total_from_user_size(0), 2);
    // user_size=1 → total=2 → exactly MIN_BLOCK_SIZE
    assert_eq!(total_from_user_size(1), 2);
}

#[test]
fn test_fsba_class_for_exact_sizes() {
    assert_eq!(fsba_class_for(2), Some(0));
    assert_eq!(fsba_class_for(4), Some(1));
    assert_eq!(fsba_class_for(8), Some(2));
    assert_eq!(fsba_class_for(16), Some(3));
    assert_eq!(fsba_class_for(32), Some(4));
}

#[test]
fn test_fsba_class_for_between_sizes() {
    // total=3 → class 1 (block_size=4)
    assert_eq!(fsba_class_for(3), Some(1));
    // total=5 → class 2 (block_size=8)
    assert_eq!(fsba_class_for(5), Some(2));
}

#[test]
fn test_fsba_class_for_too_large() {
    assert_eq!(fsba_class_for(33), None);
    assert_eq!(fsba_class_for(100), None);
}

#[test]
fn test_can_split() {
    // diff=3 >= 2 → can split
    assert!(can_split(10, 7));
    // diff=2 >= 2 → can split
    assert!(can_split(10, 8));
    // diff=1 < 2 → cannot split
    assert!(!can_split(10, 9));
    // diff=0 → cannot split
    assert!(!can_split(10, 10));
}
