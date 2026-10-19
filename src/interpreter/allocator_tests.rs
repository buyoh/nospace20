use super::*;
use InterpretError as E;

fn new_alloc() -> InterpreterAllocator {
    InterpreterAllocator::new()
}

// --- 基本 alloc / get / set ---

#[test]
fn test_alloc_basic() {
    let mut a = new_alloc();
    let ptr = a.alloc(3).unwrap();
    // ptr は 1 以上の正のアドレス
    assert!(ptr > 0);
    // get は 0 初期化
    assert_eq!(a.get(ptr).unwrap(), 0);
    assert_eq!(a.get(ptr + 1).unwrap(), 0);
    assert_eq!(a.get(ptr + 2).unwrap(), 0);
    // set / get が正常動作
    a.set(ptr, 42).unwrap();
    assert_eq!(a.get(ptr).unwrap(), 42);
    a.set(ptr + 2, 99).unwrap();
    assert_eq!(a.get(ptr + 2).unwrap(), 99);
}

#[test]
fn test_alloc_multiple() {
    let mut a = new_alloc();
    let p1 = a.alloc(3).unwrap();
    let p2 = a.alloc(3).unwrap();
    let p3 = a.alloc(5).unwrap();
    // 各ポインタは互いに異なる
    assert_ne!(p1, p2);
    assert_ne!(p2, p3);
    // 各ブロックの書き込みが独立している
    a.set(p1, 10).unwrap();
    a.set(p2, 20).unwrap();
    a.set(p3, 30).unwrap();
    assert_eq!(a.get(p1).unwrap(), 10);
    assert_eq!(a.get(p2).unwrap(), 20);
    assert_eq!(a.get(p3).unwrap(), 30);
}

#[test]
fn test_alloc_zero_size() {
    let mut a = new_alloc();
    // alloc(0) は alloc(1) と同等: 最低でも 1 つの要素にアクセス可能
    let p0 = a.alloc(0).unwrap();
    let p1 = a.alloc(1).unwrap();
    // 両方正常にアクセスできる
    a.set(p0, 7).unwrap();
    a.set(p1, 8).unwrap();
    assert_eq!(a.get(p0).unwrap(), 7);
    assert_eq!(a.get(p1).unwrap(), 8);
    // アドレスが重ならない
    assert_ne!(p0, p1);
}

// --- free ---

#[test]
fn test_free_basic() {
    let mut a = new_alloc();
    let ptr = a.alloc(3).unwrap();
    a.set(ptr, 42).unwrap();
    a.free(ptr).unwrap();
    // 解放後に get するとエラー
    assert_eq!(a.get(ptr), Err(E::FreedMemoryAccess(ptr)));
}

#[test]
fn test_free_invalid_address() {
    let mut a = new_alloc();
    // 存在しないアドレスの free はエラー
    assert_eq!(a.free(99999), Err(E::InvalidFree(99999)));
}

#[test]
fn test_double_free() {
    let mut a = new_alloc();
    let ptr = a.alloc(4).unwrap();
    a.free(ptr).unwrap();
    // 二重 free はエラー
    assert_eq!(a.free(ptr), Err(E::DoubleFree(ptr)));
}

// --- アクセスエラー ---

#[test]
fn test_access_unallocated() {
    let a = new_alloc();
    // 未割当アドレスへのアクセスはエラー
    assert_eq!(a.get(99999), Err(E::InvalidMemoryAccess(99999)));
}

#[test]
fn test_access_freed() {
    let mut a = new_alloc();
    let ptr = a.alloc(3).unwrap();
    a.free(ptr).unwrap();
    // 解放済みアドレスへのアクセスはエラー
    assert_eq!(a.get(ptr), Err(E::FreedMemoryAccess(ptr)));
}

#[test]
fn test_block_boundary() {
    let mut a = new_alloc();
    let ptr = a.alloc(3).unwrap(); // ユーザーサイズ 3 → ptr, ptr+1, ptr+2 がアクセス可能
                                   // ptr+3 はブロック境界外
    assert_eq!(a.get(ptr + 3), Err(E::InvalidMemoryAccess(ptr + 3)));
}

// --- alloc_uninit ---

#[test]
fn test_alloc_uninit_zero() {
    let mut a = new_alloc();
    let ptr = a.alloc_uninit(4, false);
    // 0 初期化
    assert_eq!(a.get(ptr).unwrap(), 0);
    assert_eq!(a.get(ptr + 1).unwrap(), 0);
    assert_eq!(a.get(ptr + 2).unwrap(), 0);
    assert_eq!(a.get(ptr + 3).unwrap(), 0);
}

#[test]
fn test_alloc_uninit_random() {
    let mut a = new_alloc();
    let ptr = a.alloc_uninit(4, true);
    // ランダムモードでは少なくとも 1 つは 0 以外の値（実装では決定論的に非 0 を保証しないが、
    // ほとんどのケースで非 0 になる）
    // ここでは panic しないことと get できることを確認
    let _ = a.get(ptr).unwrap();
    let _ = a.get(ptr + 3).unwrap();
}

// --- FSBA フリーリスト再利用 ---

#[test]
fn test_fsba_free_reuse() {
    let mut a = new_alloc();
    // FSBA クラス 0 (block_size=2, user_size=1)
    let p1 = a.alloc(1).unwrap();
    let p2 = a.alloc(1).unwrap();
    a.set(p1, 111).unwrap();
    a.set(p2, 222).unwrap();

    // p1 を解放してから再度 alloc → p1 が再利用される
    a.free(p1).unwrap();
    let p3 = a.alloc(1).unwrap();
    // p3 は p1 と同じアドレスのはず（FSBA フリーリストから再利用）
    assert_eq!(p3, p1);
    // p2 は変わらない
    assert_eq!(a.get(p2).unwrap(), 222);
}

#[test]
fn test_general_alloc_first_fit() {
    let mut a = new_alloc();
    // general alloc (user_size > 31: total > 32)
    let p1 = a.alloc(50).unwrap();
    let _p2 = a.alloc(50).unwrap();
    a.free(p1).unwrap();
    // First-Fit: p1 のブロックが再利用される
    let p3 = a.alloc(40).unwrap();
    assert_eq!(p3, p1);
}

// --- alloc_internal ---

#[test]
fn test_alloc_internal_basic() {
    let mut a = new_alloc();
    let addr = a.alloc_internal(3);
    // ヘッダーなし: addr から直接アクセス
    assert_eq!(a.get(addr).unwrap(), 0);
    assert_eq!(a.get(addr + 1).unwrap(), 0);
    assert_eq!(a.get(addr + 2).unwrap(), 0);
    a.set(addr + 1, 55).unwrap();
    assert_eq!(a.get(addr + 1).unwrap(), 55);
}

#[test]
fn test_alloc_internal_free_internal() {
    let mut a = new_alloc();
    let addr = a.alloc_internal(4);
    a.set(addr, 10).unwrap();
    a.free_internal(addr);
    // 解放後は get がエラー
    assert!(a.get(addr).is_err());
}

#[test]
fn test_alloc_internal_uninit_random() {
    let mut a = new_alloc();
    let addr = a.alloc_internal_uninit(4, true);
    // panic しないことを確認
    let _ = a.get(addr).unwrap();
    let _ = a.get(addr + 3).unwrap();
}

// --- エラーメッセージ確認（runtime_error large テストの contains と対応） ---

#[test]
fn test_error_messages() {
    let mut a = new_alloc();
    assert!(a.free(99999).unwrap_err().to_string().contains("free"));
    let ptr = a.alloc(3).unwrap();
    a.free(ptr).unwrap();
    assert!(a.free(ptr).unwrap_err().to_string().contains("double free"));
    assert!(a.get(ptr).unwrap_err().to_string().contains("freed memory"));
    assert!(a
        .get(99999)
        .unwrap_err()
        .to_string()
        .contains("invalid memory access"));
}

#[test]
fn test_free_not_block_start() {
    let mut a = new_alloc();
    let ptr = a.alloc(3).unwrap();
    assert_eq!(
        a.free(ptr + 1),
        Err(E::FreeNotBlockStart {
            addr: ptr + 1,
            block_start: ptr
        })
    );
}

// --- サイズ検証 ---

#[test]
fn test_alloc_negative_size() {
    let mut a = new_alloc();
    assert_eq!(a.alloc(-1), Err(E::InvalidAllocSize(-1)));
}

#[test]
fn test_alloc_size_limit() {
    let mut a = new_alloc();
    assert!(a.alloc(MAX_ALLOC_WORDS).is_ok());
    assert_eq!(
        a.alloc(MAX_ALLOC_WORDS + 1),
        Err(E::InvalidAllocSize(MAX_ALLOC_WORDS + 1))
    );
    assert_eq!(a.alloc(i64::MAX), Err(E::InvalidAllocSize(i64::MAX)));
}

#[test]
fn test_alloc_address_space_exhausted() {
    let mut a = new_alloc();
    // バンプポインタを上限付近に進め、checked_add の失敗がエラーになることを確認
    a.next_addr = i64::MAX - 2;
    assert_eq!(a.alloc(100), Err(E::InvalidAllocSize(100)));
}

#[test]
fn test_free_internal_reclaims_memory() {
    let mut a = new_alloc();
    let first = a.alloc_internal(4);
    a.free_internal(first);
    // 末尾ブロックの解放でアドレスが巻き戻され、同じアドレスが再利用される
    for _ in 0..1000 {
        let addr = a.alloc_internal(4);
        assert_eq!(addr, first);
        a.free_internal(addr);
    }
    assert!(a.blocks.is_empty());
}

#[test]
fn test_free_internal_nested_lifo() {
    let mut a = new_alloc();
    let outer = a.alloc_internal(2);
    let inner = a.alloc_internal(3);
    a.free_internal(inner);
    a.free_internal(outer);
    assert_eq!(a.alloc_internal(1), outer);
}
