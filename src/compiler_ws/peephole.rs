//! ピープホール最適化
//!
//! 生成された WsProgram（命令列）に対して、局所的なパターンマッチで
//! 冗長命令を除去・簡約する後処理パス。
//!
//! ## 適用パターン
//!
//! 1. `Push(x) + Discard` → 削除
//! 2. `Duplicate + Discard` → 削除
//! 3. `Push(0) + Add` → 削除（ローカル変数オフセット 0 のアドレス計算）
//! 4. ジャンプ短絡: `Jump(L1)` かつ `Label(L1)` 直後が `Jump(L2)` → `Jump(L2)`
//! 5. 到達不能コード除去: `Jump`/`Return`/`Exit` 後の非ラベル命令を削除

use crate::compiler_ws::instruction::Instruction;
use crate::compiler_ws::program::WsProgram;
use crate::compiler_ws::types::LabelId;
use std::collections::HashMap;

/// ピープホール最適化を繰り返し適用して固定点に到達するまで実行
pub fn optimize(prog: WsProgram) -> WsProgram {
    let mut instructions = prog.into_instructions();
    loop {
        let (new_instructions, changed) = apply_patterns(instructions);
        instructions = new_instructions;
        if !changed {
            break;
        }
    }
    WsProgram::from_instructions(instructions)
}

fn apply_patterns(instructions: Vec<Instruction>) -> (Vec<Instruction>, bool) {
    // パターン 4 用: Label(L) の直後（Labels を除く最初の命令）が Jump(M) のとき L → M マップを構築
    let jump_forward_map = build_jump_forward_map(&instructions);

    let mut result = Vec::with_capacity(instructions.len());
    let mut changed = false;
    let mut i = 0;

    while i < instructions.len() {
        let inst = &instructions[i];

        // パターン 1: Push(x) + Discard → 削除
        if matches!(inst, Instruction::Push(_)) {
            if i + 1 < instructions.len() && matches!(instructions[i + 1], Instruction::Discard) {
                i += 2;
                changed = true;
                continue;
            }
        }

        // パターン 2: Duplicate + Discard → 削除
        if matches!(inst, Instruction::Duplicate) {
            if i + 1 < instructions.len() && matches!(instructions[i + 1], Instruction::Discard) {
                i += 2;
                changed = true;
                continue;
            }
        }

        // パターン 3: Push(0) + Add → 削除
        if let Instruction::Push(n) = inst {
            if n.0 == 0 && i + 1 < instructions.len() && matches!(instructions[i + 1], Instruction::Add) {
                i += 2;
                changed = true;
                continue;
            }
        }

        // パターン 4: ジャンプ短絡
        // Jump(L1) で L1 → L2 のマッピングがある場合、Jump(L2) に変換
        // Call はサブルーチン呼び出しのため対象外
        if let Some((new_inst, shortcut_changed)) = try_shortcut_jump(inst, &jump_forward_map) {
            result.push(new_inst.clone());
            i += 1;
            if shortcut_changed {
                changed = true;
            }
            // パターン 4 に加えてパターン 5 も適用:
            // 無条件ジャンプ後の到達不能コードを除去
            if matches!(new_inst, Instruction::Jump(_)) {
                while i < instructions.len() {
                    if let Instruction::Label(_) = &instructions[i] {
                        break;
                    }
                    i += 1;
                    changed = true;
                }
            }
            continue;
        }

        // パターン 5: 到達不能コードの除去
        // Return / Exit 後、次のラベルまでの命令を除去
        // ※ Jump(_) は上の try_shortcut_jump で処理済み
        let is_terminator = matches!(inst, Instruction::Return | Instruction::Exit);
        result.push(inst.clone());
        i += 1;

        if is_terminator {
            // 次のラベルまでの命令をスキップ（ラベルがあれば到達可能なので停止）
            while i < instructions.len() {
                if let Instruction::Label(_) = &instructions[i] {
                    break;
                }
                i += 1;
                changed = true;
            }
        }
    }

    (result, changed)
}

/// Label(L) の直後（連続するラベルを除く最初の命令）が Jump(M) のとき L → M マップを構築
///
/// 連鎖する場合（L1→L2→L3）は最終的な宛先まで追跡する（サイクル検出付き）
fn build_jump_forward_map(instructions: &[Instruction]) -> HashMap<LabelId, LabelId> {
    // 1パスで直接マッピングを収集
    let mut direct_map: HashMap<LabelId, LabelId> = HashMap::new();
    let mut i = 0;
    while i < instructions.len() {
        if let Instruction::Label(label_id) = &instructions[i] {
            // ラベルの後の非ラベル命令を探す
            let mut j = i + 1;
            while j < instructions.len() {
                match &instructions[j] {
                    Instruction::Label(_) => {
                        j += 1; // 連続するラベルはスキップ
                    }
                    Instruction::Jump(target) => {
                        direct_map.insert(*label_id, *target);
                        break;
                    }
                    _ => break,
                }
            }
        }
        i += 1;
    }

    // 連鎖を解決（L1→L2→L3 → L1→L3）
    let mut resolved_map: HashMap<LabelId, LabelId> = HashMap::new();
    for &label in direct_map.keys() {
        let mut current = label;
        let mut visited = std::collections::HashSet::new();
        loop {
            if visited.contains(&current) {
                // サイクル検出: 解決不能
                break;
            }
            visited.insert(current);
            if let Some(&next) = direct_map.get(&current) {
                current = next;
            } else {
                // 最終宛先
                if current != label {
                    resolved_map.insert(label, current);
                }
                break;
            }
        }
    }

    // 直接マップと解決済みマップをマージ（解決済みを優先）
    let mut result = direct_map;
    for (k, v) in resolved_map {
        result.insert(k, v);
    }
    result
}

/// ジャンプ命令のターゲットを短絡する（パターン 4）
///
/// 変換が適用された場合は `Some((新命令, true))`、
/// パターン4対象の命令でも変換なしなら `Some((元命令, false))`、
/// パターン4対象外（Label など）は `None` を返す
fn try_shortcut_jump(
    inst: &Instruction,
    map: &HashMap<LabelId, LabelId>,
) -> Option<(Instruction, bool)> {
    match inst {
        Instruction::Jump(target) => {
            if let Some(&new_target) = map.get(target) {
                if new_target != *target {
                    return Some((Instruction::Jump(new_target), true));
                }
            }
            Some((inst.clone(), false))
        }
        Instruction::JumpIfZero(target) => {
            if let Some(&new_target) = map.get(target) {
                if new_target != *target {
                    return Some((Instruction::JumpIfZero(new_target), true));
                }
            }
            Some((inst.clone(), false))
        }
        Instruction::JumpIfNegative(target) => {
            if let Some(&new_target) = map.get(target) {
                if new_target != *target {
                    return Some((Instruction::JumpIfNegative(new_target), true));
                }
            }
            Some((inst.clone(), false))
        }
        // Call はサブルーチン呼び出しのためジャンプ短絡の対象外
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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
        let prog = make_prog(vec![
            Instruction::Push(WsNumber(1)),
            Instruction::Add,
        ]);
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
            Instruction::Add,               // 到達不能
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
}
