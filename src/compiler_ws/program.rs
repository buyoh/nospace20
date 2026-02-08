//! Whitespace プログラム構造

use crate::compiler_ws::encoder;
use crate::compiler_ws::instruction::Instruction;

/// Whitespace プログラム（命令列）
#[derive(Debug, Clone)]
pub struct WsProgram {
    instructions: Vec<Instruction>,
}

impl WsProgram {
    /// 新しい空のプログラムを作成
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }

    /// 命令を追加
    pub fn push(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    /// 命令列を追加
    pub fn extend<I>(&mut self, insts: I)
    where
        I: IntoIterator<Item = Instruction>,
    {
        self.instructions.extend(insts);
    }

    /// 別のプログラムを追加
    pub fn append(&mut self, other: WsProgram) {
        self.instructions.extend(other.instructions);
    }

    /// Whitespace コード文字列に変換
    pub fn to_whitespace(&self) -> String {
        let mut chars = Vec::new();
        for inst in &self.instructions {
            chars.extend(inst.encode());
        }
        encoder::to_string(&chars)
    }

    /// デバッグ用のニーモニック表現
    pub fn to_debug_string(&self) -> String {
        self.instructions
            .iter()
            .map(|inst| inst.to_mnemonic())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 命令数を取得
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// 空かどうかを判定
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// 命令列を消費して Vec<Instruction> を返す
    /// WhitespaceVM へ渡す際に使用
    pub fn into_instructions(self) -> Vec<Instruction> {
        self.instructions
    }

    /// 命令列への参照を返す
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
}

impl Default for WsProgram {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_ws::types::{LabelId, WsNumber};

    #[test]
    fn test_program_creation() {
        let mut prog = WsProgram::new();
        assert!(prog.is_empty());

        prog.push(Instruction::Push(WsNumber(42)));
        assert_eq!(prog.len(), 1);
    }

    #[test]
    fn test_program_extend() {
        let mut prog = WsProgram::new();
        prog.extend([
            Instruction::Push(WsNumber(1)),
            Instruction::Push(WsNumber(2)),
            Instruction::Add,
        ]);
        assert_eq!(prog.len(), 3);
    }

    #[test]
    fn test_program_append() {
        let mut prog1 = WsProgram::new();
        prog1.push(Instruction::Push(WsNumber(1)));

        let mut prog2 = WsProgram::new();
        prog2.push(Instruction::Push(WsNumber(2)));

        prog1.append(prog2);
        assert_eq!(prog1.len(), 2);
    }

    #[test]
    fn test_to_whitespace() {
        let mut prog = WsProgram::new();
        prog.push(Instruction::Exit);

        let ws = prog.to_whitespace();
        // Exit = LF LF LF
        assert_eq!(ws, "\n\n\n");
    }
}
