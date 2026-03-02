use super::*;

#[test]
fn test_memory_layout_constants() {
    assert_eq!(MemoryLayout::LOCAL_HEAP_BEGIN.value(), 2);
    assert_eq!(MemoryLayout::LOCAL_HEAP_END.value(), 3);
    assert_eq!(MemoryLayout::TEMP_PTR.value(), 4);
    assert_eq!(MemoryLayout::GLOBAL_PTR.value(), 8);
}

#[test]
fn test_allocate_global() {
    let mut layout = MemoryLayout::new();

    let addr1 = layout.allocate_global();
    assert_eq!(addr1.value(), 8);

    let addr2 = layout.allocate_global();
    assert_eq!(addr2.value(), 9);

    assert_eq!(layout.global_size(), 2);
}

#[test]
fn test_initial_local_heap() {
    let mut layout = MemoryLayout::new();
    layout.allocate_global();
    layout.allocate_global();

    let local_start = layout.initial_local_heap();
    assert_eq!(local_start.value(), 10); // 8 + 2
}
