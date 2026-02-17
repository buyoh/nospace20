//! Whitespace 命令定義

use crate::compiler_ws::types::{LabelId, WsChar, WsNumber};

/// Whitespace 命令
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // === スタック操作 (IMP: SP) ===
    Push(WsNumber), // SP SP <number>
    Duplicate,      // SP LF SP
    Copy(WsNumber), // SP TB SP <n>
    Swap,           // SP LF TB
    Discard,        // SP LF LF

    // === 算術演算 (IMP: TB SP) ===
    Add, // TB SP SP SP
    Sub, // TB SP SP TB
    Mul, // TB SP SP LF
    Div, // TB SP TB SP
    Mod, // TB SP TB TB

    // === ヒープアクセス (IMP: TB TB) ===
    Store,    // TB TB SP
    Retrieve, // TB TB TB

    // === フロー制御 (IMP: LF) ===
    Label(LabelId),          // LF SP SP <label>
    Call(LabelId),           // LF SP TB <label>
    Jump(LabelId),           // LF SP LF <label>
    JumpIfZero(LabelId),     // LF TB SP <label>
    JumpIfNegative(LabelId), // LF TB TB <label>
    Return,                  // LF TB LF
    Exit,                    // LF LF LF

    // === I/O (IMP: TB LF) ===
    OutputChar,   // TB LF SP SP
    OutputNumber, // TB LF SP TB
    InputChar,    // TB LF TB SP
    InputNumber,  // TB LF TB TB
}

impl Instruction {
    pub fn encode(&self) -> Vec<WsChar> {
        use Instruction::*;
        use WsChar::*;

        match self {
            // スタック操作
            Push(n) => {
                let mut v = vec![Space, Space];
                v.extend(n.encode());
                v
            }
            Duplicate => vec![Space, Lf, Space],
            Copy(n) => {
                let mut v = vec![Space, Tab, Space];
                v.extend(n.encode());
                v
            }
            Swap => vec![Space, Lf, Tab],
            Discard => vec![Space, Lf, Lf],

            // 算術演算
            Add => vec![Tab, Space, Space, Space],
            Sub => vec![Tab, Space, Space, Tab],
            Mul => vec![Tab, Space, Space, Lf],
            Div => vec![Tab, Space, Tab, Space],
            Mod => vec![Tab, Space, Tab, Tab],

            // ヒープアクセス
            Store => vec![Tab, Tab, Space],
            Retrieve => vec![Tab, Tab, Tab],

            // フロー制御
            Label(id) => {
                let mut v = vec![Lf, Space, Space];
                v.extend(WsNumber(id.to_ws_value()).encode());
                v
            }
            Call(id) => {
                let mut v = vec![Lf, Space, Tab];
                v.extend(WsNumber(id.to_ws_value()).encode());
                v
            }
            Jump(id) => {
                let mut v = vec![Lf, Space, Lf];
                v.extend(WsNumber(id.to_ws_value()).encode());
                v
            }
            JumpIfZero(id) => {
                let mut v = vec![Lf, Tab, Space];
                v.extend(WsNumber(id.to_ws_value()).encode());
                v
            }
            JumpIfNegative(id) => {
                let mut v = vec![Lf, Tab, Tab];
                v.extend(WsNumber(id.to_ws_value()).encode());
                v
            }
            Return => vec![Lf, Tab, Lf],
            Exit => vec![Lf, Lf, Lf],

            // I/O
            OutputChar => vec![Tab, Lf, Space, Space],
            OutputNumber => vec![Tab, Lf, Space, Tab],
            InputChar => vec![Tab, Lf, Tab, Space],
            InputNumber => vec![Tab, Lf, Tab, Tab],
        }
    }

    /// デバッグ用のニーモニック表現
    pub fn to_mnemonic(&self) -> String {
        use Instruction::*;
        match self {
            Push(n) => format!("push {}", n.0),
            Duplicate => "dup".to_string(),
            Copy(n) => format!("copy {}", n.0),
            Swap => "swap".to_string(),
            Discard => "pop".to_string(),
            Add => "add".to_string(),
            Sub => "sub".to_string(),
            Mul => "mul".to_string(),
            Div => "div".to_string(),
            Mod => "mod".to_string(),
            Store => "set".to_string(),
            Retrieve => "get".to_string(),
            Label(id) => format!("label_{}:", id.0),
            Call(id) => format!("call label_{}", id.0),
            Jump(id) => format!("jmp label_{}", id.0),
            JumpIfZero(id) => format!("jz label_{}", id.0),
            JumpIfNegative(id) => format!("jn label_{}", id.0),
            Return => "ret".to_string(),
            Exit => "exit".to_string(),
            OutputChar => "pchr".to_string(),
            OutputNumber => "pnum".to_string(),
            InputChar => "ichr".to_string(),
            InputNumber => "inum".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_push() {
        let inst = Instruction::Push(WsNumber(1));
        let encoded = inst.encode();
        // SP SP [number encoding]
        assert_eq!(encoded[0], WsChar::Space);
        assert_eq!(encoded[1], WsChar::Space);
    }

    #[test]
    fn test_encode_add() {
        let inst = Instruction::Add;
        assert_eq!(
            inst.encode(),
            vec![WsChar::Tab, WsChar::Space, WsChar::Space, WsChar::Space]
        );
    }

    #[test]
    fn test_encode_label() {
        let inst = Instruction::Label(LabelId(16));
        let encoded = inst.encode();
        assert_eq!(encoded[0], WsChar::Lf);
        assert_eq!(encoded[1], WsChar::Space);
        assert_eq!(encoded[2], WsChar::Space);
    }
}
