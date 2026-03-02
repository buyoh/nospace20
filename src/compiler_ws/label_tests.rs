use super::*;

#[test]
fn test_label_allocator() {
    let mut alloc = LabelAllocator::new();
    let l1 = alloc.allocate();
    let l2 = alloc.allocate();
    assert_eq!(l1.0, 48);
    assert_eq!(l2.0, 49);
}

#[test]
fn test_label_range() {
    let mut alloc = LabelAllocator::new();
    let base = alloc.allocate_range(5);
    assert_eq!(base.0, 48);

    let next = alloc.allocate();
    assert_eq!(next.0, 53);
}

#[test]
fn test_function_label() {
    let mut alloc = LabelAllocator::new();

    let f1 = alloc.get_or_create_function_label(0);
    assert_eq!(f1.0, 48);

    let f1_again = alloc.get_or_create_function_label(0);
    assert_eq!(f1_again.0, 48); // 同じラベル

    let f2 = alloc.get_or_create_function_label(1);
    assert_eq!(f2.0, 50); // 0 が 48-49 を使用、1 は 50-51
}

#[test]
fn test_has_function() {
    let mut alloc = LabelAllocator::new();

    assert!(!alloc.has_function(0));
    alloc.get_or_create_function_label(0);
    assert!(alloc.has_function(0));
}

#[test]
fn test_sync_next_id() {
    let mut parent = LabelAllocator::new();
    parent.allocate(); // next_id = 49

    let mut child = parent.clone();
    child.allocate(); // child next_id = 50
    child.allocate(); // child next_id = 51

    parent.sync_next_id(&child);
    assert_eq!(parent.allocate().0, 51); // 同期後は 51 から割り当て
}

#[test]
fn test_sync_function_labels() {
    let mut parent = LabelAllocator::new();
    parent.get_or_create_function_label(0); // label_48

    let mut child = parent.clone();
    child.get_or_create_function_label(1); // label_50

    parent.sync_next_id(&child);

    // 子で作成された関数ラベルが親にマージされることを確認
    assert_eq!(parent.get_function_label(1), Some(LabelId(50)));
    // 次のラベルは 52 から
    assert_eq!(parent.allocate().0, 52);
}

#[test]
fn test_sync_no_change_when_child_behind() {
    let mut parent = LabelAllocator::new();
    parent.allocate(); // next_id = 49
    parent.allocate(); // next_id = 50
    parent.allocate(); // next_id = 51

    let child = parent.clone();
    // 子では何も割り当てない (next_id = 51 のまま)

    parent.sync_next_id(&child);
    // 子の next_id が親と同じなので変化なし
    assert_eq!(parent.allocate().0, 51);
}

#[test]
fn test_sync_multiple_children() {
    let mut parent = LabelAllocator::new();
    parent.allocate(); // next_id = 49

    // 第1の子コンテキスト
    let mut child1 = parent.clone();
    child1.allocate(); // child1 next_id = 50
    parent.sync_next_id(&child1);

    // 第2の子コンテキスト
    let mut child2 = parent.clone();
    child2.allocate(); // child2 next_id = 51
    child2.allocate(); // child2 next_id = 52
    parent.sync_next_id(&child2);

    // 最終的に child2 の next_id まで同期される
    assert_eq!(parent.allocate().0, 52);
}

#[test]
fn test_multi_function_simulation() {
    // 実際のバグシナリオをシミュレーション:
    // 関数1で制御構造のラベルを使用し、その後関数2を定義
    let mut parent = LabelAllocator::new();

    // 関数0 (puts) を定義開始
    let func1_label = parent.get_or_create_function_label(0); // label_48, 49
    assert_eq!(func1_label.0, 48);

    // 関数0の本体を生成 (子コンテキスト)
    let mut child1 = parent.clone();
    let loop_start = child1.allocate(); // label_50 (while ループ開始)
    let loop_end = child1.allocate(); // label_51 (while ループ終了)
    assert_eq!(loop_start.0, 50);
    assert_eq!(loop_end.0, 51);

    // 関数0完了後、ラベルを同期 ← これがバグ修正
    parent.sync_next_id(&child1);

    // 関数1 (main) を定義
    let func2_label = parent.get_or_create_function_label(1); // label_52, 53
    assert_eq!(func2_label.0, 52); // label_50 ではなく label_52 (重複回避成功!)

    // 関数1の本体
    let mut child2 = parent.clone();
    let if_else_label = child2.allocate(); // label_54
    assert_eq!(if_else_label.0, 54);

    parent.sync_next_id(&child2);

    // 最終的に全てのラベルが一意であることを確認
    assert_eq!(parent.allocate().0, 55);
}

#[test]
fn test_shadowed_function_labels_are_unique() {
    // 関数シャドーイングのシナリオ:
    // 同名の関数が異なるスコープに存在する場合、異なるラベルが割り当てられるべき。
    // 例: グローバルの foo (index=0) とネストされた foo (index=2) は別ラベル。
    let mut alloc = LabelAllocator::new();

    // グローバル foo (func_index=0) → label_48, 49
    let foo_global = alloc.get_or_create_function_label(0);
    assert_eq!(foo_global.0, 48);

    // outer (func_index=1) → label_50, 51
    let outer = alloc.get_or_create_function_label(1);
    assert_eq!(outer.0, 50);

    // ネストされた foo (func_index=2) → label_52, 53 (重複しない!)
    let foo_nested = alloc.get_or_create_function_label(2);
    assert_eq!(foo_nested.0, 52);

    // グローバル foo を再度取得しても同じラベル
    let foo_global_again = alloc.get_or_create_function_label(0);
    assert_eq!(foo_global_again.0, 48);

    // ネストされた foo を再度取得しても同じラベル
    let foo_nested_again = alloc.get_or_create_function_label(2);
    assert_eq!(foo_nested_again.0, 52);

    // 全ラベルが一意であることを確認
    assert_ne!(foo_global, foo_nested);
}
