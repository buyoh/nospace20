use super::*;

fn new_alloc() -> InterpreterAllocator {
    InterpreterAllocator::new()
}

// --- 基本 alloc / get / set ---

#[test]
fn test_alloc_basic() {
    let mut a = new_alloc();
    let ptr = a.alloc(3);
    // ptr は 1 以上の正のアドレス
    assert!(ptr > 0);
    // get は 0 初期化
    assert_eq!(a.get(ptr), 0);
    assert_eq!(a.get(ptr + 1), 0);
    assert_eq!(a.get(ptr + 2), 0);
    // set / get が正常動作
    a.set(ptr, 42);
    assert_eq!(a.get(ptr), 42);
    a.set(ptr + 2, 99);
    assert_eq!(a.get(ptr + 2), 99);
}

#[test]
fn test_alloc_multiple() {
    let mut a = new_alloc();
    let p1 = a.alloc(3);
    let p2 = a.alloc(3);
    let p3 = a.alloc(5);
    // 各ポインタは互いに異なる
    assert_ne!(p1, p2);
    assert_ne!(p2, p3);
    // 各ブロックの書き込みが独立している
    a.set(p1, 10);
    a.set(p2, 20);
    a.set(p3, 30);
    assert_eq!(a.get(p1), 10);
    assert_eq!(a.get(p2), 20);
    assert_eq!(a.get(p3), 30);
}

#[test]
fn test_alloc_zero_size() {
    let mut a = new_alloc();
    // alloc(0) は alloc(1) と同等: 最低でも 1 つの要素にアクセス可能
    let p0 = a.alloc(0);
    let p1 = a.alloc(1);
    // 両方正常にアクセスできる
    a.set(p0, 7);
    a.set(p1, 8);
    assert_eq!(a.get(p0), 7);
    assert_eq!(a.get(p1), 8);
    // アドレスが重ならない
    assert_ne!(p0, p1);
}

// --- free ---

#[test]
fn test_free_basic() {
    let mut a = new_alloc();
    let ptr = a.alloc(3);
    a.set(ptr, 42);
    a.free(ptr);
    // 解放後に get すると panic
    let result = std::panic::catch_unwind(move || a.get(ptr));
    assert!(result.is_err());
}

#[test]
fn test_free_invalid_address() {
    let mut a = new_alloc();
    // 存在しないアドレスの free は panic
    let result = std::panic::catch_unwind(move || a.free(99999));
    assert!(result.is_err());
}

#[test]
fn test_double_free() {
    let mut a = new_alloc();
    let ptr = a.alloc(4);
    a.free(ptr);
    // 二重 free は panic
    let result = std::panic::catch_unwind(move || a.free(ptr));
    assert!(result.is_err());
}

// --- アクセスエラー ---

#[test]
fn test_access_unallocated() {
    let a = new_alloc();
    // 未割当アドレスへのアクセスは panic
    let result = std::panic::catch_unwind(move || a.get(99999));
    assert!(result.is_err());
}

#[test]
fn test_access_freed() {
    let mut a = new_alloc();
    let ptr = a.alloc(3);
    a.free(ptr);
    // 解放済みアドレスへのアクセスは panic
    let result = std::panic::catch_unwind(move || a.get(ptr));
    assert!(result.is_err());
}

#[test]
fn test_block_boundary() {
    let mut a = new_alloc();
    let ptr = a.alloc(3); // ユーザーサイズ 3 → ptr, ptr+1, ptr+2 がアクセス可能
                          // ptr+3 はブロック境界外
    let result = std::panic::catch_unwind(move || a.get(ptr + 3));
    assert!(result.is_err());
}

// --- alloc_uninit ---

#[test]
fn test_alloc_uninit_zero() {
    let mut a = new_alloc();
    let ptr = a.alloc_uninit(4, false);
    // 0 初期化
    assert_eq!(a.get(ptr), 0);
    assert_eq!(a.get(ptr + 1), 0);
    assert_eq!(a.get(ptr + 2), 0);
    assert_eq!(a.get(ptr + 3), 0);
}

#[test]
fn test_alloc_uninit_random() {
    let mut a = new_alloc();
    let ptr = a.alloc_uninit(4, true);
    // ランダムモードでは少なくとも 1 つは 0 以外の値（実装では決定論的に非 0 を保証しないが、
    // ほとんどのケースで非 0 になる）
    // ここでは panic しないことと get できることを確認
    let _ = a.get(ptr);
    let _ = a.get(ptr + 3);
}

// --- FSBA フリーリスト再利用 ---

#[test]
fn test_fsba_free_reuse() {
    let mut a = new_alloc();
    // FSBA クラス 0 (block_size=2, user_size=1)
    let p1 = a.alloc(1);
    let p2 = a.alloc(1);
    a.set(p1, 111);
    a.set(p2, 222);

    // p1 を解放してから再度 alloc → p1 が再利用される
    a.free(p1);
    let p3 = a.alloc(1);
    // p3 は p1 と同じアドレスのはず（FSBA フリーリストから再利用）
    assert_eq!(p3, p1);
    // p2 は変わらない
    assert_eq!(a.get(p2), 222);
}

#[test]
fn test_general_alloc_first_fit() {
    let mut a = new_alloc();
    // general alloc (user_size > 31: total > 32)
    let p1 = a.alloc(50);
    let _p2 = a.alloc(50);
    a.free(p1);
    // First-Fit: p1 のブロックが再利用される
    let p3 = a.alloc(40);
    assert_eq!(p3, p1);
}

// --- alloc_internal ---

#[test]
fn test_alloc_internal_basic() {
    let mut a = new_alloc();
    let addr = a.alloc_internal(3);
    // ヘッダーなし: addr から直接アクセス
    assert_eq!(a.get(addr), 0);
    assert_eq!(a.get(addr + 1), 0);
    assert_eq!(a.get(addr + 2), 0);
    a.set(addr + 1, 55);
    assert_eq!(a.get(addr + 1), 55);
}

#[test]
fn test_alloc_internal_free_internal() {
    let mut a = new_alloc();
    let addr = a.alloc_internal(4);
    a.set(addr, 10);
    a.free_internal(addr);
    // 解放後は get が panic
    let result = std::panic::catch_unwind(move || a.get(addr));
    assert!(result.is_err());
}

#[test]
fn test_alloc_internal_uninit_random() {
    let mut a = new_alloc();
    let addr = a.alloc_internal_uninit(4, true);
    // panic しないことを確認
    let _ = a.get(addr);
    let _ = a.get(addr + 3);
}

// --- エラーメッセージ確認 ---

#[test]
fn test_free_invalid_address_message() {
    let mut a = new_alloc();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.free(99999)));
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(msg.contains("free"), "expected 'free' in error, got: {msg}");
}

#[test]
fn test_double_free_message() {
    let mut a = new_alloc();
    let ptr = a.alloc(3);
    a.free(ptr);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.free(ptr)));
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(
        msg.contains("double free"),
        "expected 'double free' in error, got: {msg}"
    );
}

#[test]
fn test_access_freed_message() {
    let mut a = new_alloc();
    let ptr = a.alloc(3);
    a.free(ptr);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.get(ptr)));
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(
        msg.contains("freed memory"),
        "expected 'freed memory' in error, got: {msg}"
    );
}

#[test]
fn test_access_unallocated_message() {
    let a = new_alloc();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.get(99999)));
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(
        msg.contains("invalid memory access"),
        "expected 'invalid memory access' in error, got: {msg}"
    );
}
