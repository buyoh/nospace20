use super::*;
use crate::compiler_ws::instruction::Instruction;
use crate::compiler_ws::program::WsProgram;
use crate::compiler_ws::types::{LabelId, WsNumber};

fn make_prog(insts: Vec<Instruction>) -> WsProgram {
    let mut prog = WsProgram::new();
    for inst in insts {
        prog.push(inst);
    }
    prog
}

/// パターン 1: Push(x) + Discard → 削除
#[test]
fn test_pattern1_push_discard() {
    let prog = make_prog(vec![
        Instruction::Push(WsNumber(42)),
        Instruction::Discard,
        Instruction::Exit,
    ]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    assert_eq!(insts.len(), 1);
    assert!(matches!(insts[0], Instruction::Exit));
}

/// パターン 2: Duplicate + Discard → 削除
#[test]
fn test_pattern2_duplicate_discard() {
    let prog = make_prog(vec![
        Instruction::Duplicate,
        Instruction::Discard,
        Instruction::Exit,
    ]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    assert_eq!(insts.len(), 1);
    assert!(matches!(insts[0], Instruction::Exit));
}

/// パターン 3: Push(0) + Add → 削除
#[test]
fn test_pattern3_push0_add() {
    let prog = make_prog(vec![
        Instruction::Push(WsNumber(0)),
        Instruction::Add,
        Instruction::Exit,
    ]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    assert_eq!(insts.len(), 1);
    assert!(matches!(insts[0], Instruction::Exit));
}

/// パターン 3: Push(0) 以外は削除しない
#[test]
fn test_pattern3_push_nonzero_add_kept() {
    let prog = make_prog(vec![Instruction::Push(WsNumber(1)), Instruction::Add]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    assert_eq!(insts.len(), 2);
}

/// パターン 4: ジャンプ短絡
#[test]
fn test_pattern4_jump_shortcut() {
    let l1 = LabelId(100);
    let l2 = LabelId(101);
    let prog = make_prog(vec![
        Instruction::Jump(l1),
        Instruction::Label(l1),
        Instruction::Jump(l2),
    ]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    // Jump(l1) が Jump(l2) に短絡される
    assert!(matches!(insts[0], Instruction::Jump(id) if id == l2));
}

/// パターン 5: 到達不能コードの除去
#[test]
fn test_pattern5_unreachable_code() {
    let l1 = LabelId(100);
    let prog = make_prog(vec![
        Instruction::Jump(l1),
        Instruction::Push(WsNumber(99)), // 到達不能
        Instruction::Add,                // 到達不能
        Instruction::Label(l1),
        Instruction::Exit,
    ]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    // Jump + Label + Exit の 3 命令だけになる
    assert_eq!(insts.len(), 3);
    assert!(matches!(insts[0], Instruction::Jump(_)));
    assert!(matches!(insts[1], Instruction::Label(_)));
    assert!(matches!(insts[2], Instruction::Exit));
}

/// Return 後の到達不能コードも除去される
#[test]
fn test_pattern5_unreachable_after_return() {
    let l1 = LabelId(100);
    let prog = make_prog(vec![
        Instruction::Return,
        Instruction::Push(WsNumber(1)), // 到達不能
        Instruction::Label(l1),         // ラベルがあれば停止
        Instruction::Exit,
    ]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    assert_eq!(insts.len(), 3);
    assert!(matches!(insts[0], Instruction::Return));
    assert!(matches!(insts[1], Instruction::Label(_)));
    assert!(matches!(insts[2], Instruction::Exit));
}

/// パターン適用なし: 変更されない命令列
#[test]
fn test_no_pattern_match() {
    let prog = make_prog(vec![
        Instruction::Push(WsNumber(1)),
        Instruction::Push(WsNumber(2)),
        Instruction::Add,
        Instruction::OutputNumber,
        Instruction::Exit,
    ]);
    let result = optimize(prog);
    let insts = result.into_instructions();
    assert_eq!(insts.len(), 5);
}
