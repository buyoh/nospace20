//! Whitespace パーサ
//!
//! Whitespace テキスト（Space / Tab / LF のシーケンス）を `Vec<Instruction>` にパースする

use crate::compiler_ws::instruction::Instruction;
use crate::compiler_ws::types::{LabelId, WsChar, WsNumber};

/// パースエラー
///
/// NOTE: 型定義は `base::error::ws_error::WsParseError` に移動。
/// 後方互換のため type alias として公開。
pub use crate::base::error::ws_error::WsParseError;
pub type ParseError = WsParseError;

/// Whitespace テキストを命令列にパースする
pub fn parse(source: &str) -> Result<Vec<Instruction>, ParseError> {
    let chars: Vec<WsChar> = source
        .chars()
        .filter_map(|c| match c {
            ' ' => Some(WsChar::Space),
            '\t' => Some(WsChar::Tab),
            '\n' => Some(WsChar::Lf),
            _ => None, // Space/Tab/LF 以外は無視
        })
        .collect();

    let mut pos = 0;
    let mut instructions = Vec::new();

    while pos < chars.len() {
        let (inst, new_pos) = parse_instruction(&chars, pos)?;
        instructions.push(inst);
        pos = new_pos;
    }

    Ok(instructions)
}

/// 1命令をパース
fn parse_instruction(chars: &[WsChar], pos: usize) -> Result<(Instruction, usize), ParseError> {
    match chars.get(pos) {
        Some(WsChar::Space) => parse_stack_op(chars, pos + 1),
        Some(WsChar::Tab) => match chars.get(pos + 1) {
            Some(WsChar::Space) => parse_arithmetic(chars, pos + 2),
            Some(WsChar::Tab) => parse_heap(chars, pos + 2),
            Some(WsChar::Lf) => parse_io(chars, pos + 2),
            None => Err(ParseError::UnexpectedEof {
                context: "after Tab IMP".into(),
            }),
        },
        Some(WsChar::Lf) => parse_flow(chars, pos + 1),
        None => Err(ParseError::UnexpectedEof {
            context: "instruction start".into(),
        }),
    }
}

/// スタック操作命令をパース (IMP: [S])
fn parse_stack_op(chars: &[WsChar], pos: usize) -> Result<(Instruction, usize), ParseError> {
    match (chars.get(pos), chars.get(pos + 1)) {
        (Some(WsChar::Space), _) => {
            // Push <number>
            let (n, new_pos) = parse_number(chars, pos + 1)?;
            Ok((Instruction::Push(n), new_pos))
        }
        (Some(WsChar::Lf), Some(WsChar::Space)) => {
            // Duplicate
            Ok((Instruction::Duplicate, pos + 2))
        }
        (Some(WsChar::Lf), Some(WsChar::Tab)) => {
            // Swap
            Ok((Instruction::Swap, pos + 2))
        }
        (Some(WsChar::Lf), Some(WsChar::Lf)) => {
            // Discard
            Ok((Instruction::Discard, pos + 2))
        }
        (Some(WsChar::Tab), Some(WsChar::Space)) => {
            // Copy <n>
            let (n, new_pos) = parse_number(chars, pos + 2)?;
            Ok((Instruction::Copy(n), new_pos))
        }
        _ => Err(ParseError::InvalidCommand {
            position: pos,
            imp: "Stack".into(),
        }),
    }
}

/// 算術演算命令をパース (IMP: [T][S])
fn parse_arithmetic(chars: &[WsChar], pos: usize) -> Result<(Instruction, usize), ParseError> {
    match (chars.get(pos), chars.get(pos + 1)) {
        (Some(WsChar::Space), Some(WsChar::Space)) => {
            // Add
            Ok((Instruction::Add, pos + 2))
        }
        (Some(WsChar::Space), Some(WsChar::Tab)) => {
            // Sub
            Ok((Instruction::Sub, pos + 2))
        }
        (Some(WsChar::Space), Some(WsChar::Lf)) => {
            // Mul
            Ok((Instruction::Mul, pos + 2))
        }
        (Some(WsChar::Tab), Some(WsChar::Space)) => {
            // Div
            Ok((Instruction::Div, pos + 2))
        }
        (Some(WsChar::Tab), Some(WsChar::Tab)) => {
            // Mod
            Ok((Instruction::Mod, pos + 2))
        }
        _ => Err(ParseError::InvalidCommand {
            position: pos,
            imp: "Arithmetic".into(),
        }),
    }
}

/// ヒープアクセス命令をパース (IMP: [T][T])
fn parse_heap(chars: &[WsChar], pos: usize) -> Result<(Instruction, usize), ParseError> {
    match chars.get(pos) {
        Some(WsChar::Space) => {
            // Store
            Ok((Instruction::Store, pos + 1))
        }
        Some(WsChar::Tab) => {
            // Retrieve
            Ok((Instruction::Retrieve, pos + 1))
        }
        _ => Err(ParseError::InvalidCommand {
            position: pos,
            imp: "Heap".into(),
        }),
    }
}

/// I/O 命令をパース (IMP: [T][LF])
fn parse_io(chars: &[WsChar], pos: usize) -> Result<(Instruction, usize), ParseError> {
    match (chars.get(pos), chars.get(pos + 1)) {
        (Some(WsChar::Space), Some(WsChar::Space)) => {
            // OutputChar
            Ok((Instruction::OutputChar, pos + 2))
        }
        (Some(WsChar::Space), Some(WsChar::Tab)) => {
            // OutputNumber
            Ok((Instruction::OutputNumber, pos + 2))
        }
        (Some(WsChar::Tab), Some(WsChar::Space)) => {
            // InputChar
            Ok((Instruction::InputChar, pos + 2))
        }
        (Some(WsChar::Tab), Some(WsChar::Tab)) => {
            // InputNumber
            Ok((Instruction::InputNumber, pos + 2))
        }
        _ => Err(ParseError::InvalidCommand {
            position: pos,
            imp: "IO".into(),
        }),
    }
}

/// フロー制御命令をパース (IMP: [LF])
fn parse_flow(chars: &[WsChar], pos: usize) -> Result<(Instruction, usize), ParseError> {
    match (chars.get(pos), chars.get(pos + 1)) {
        (Some(WsChar::Space), Some(WsChar::Space)) => {
            // Label <label>
            let (id, new_pos) = parse_label(chars, pos + 2)?;
            Ok((Instruction::Label(id), new_pos))
        }
        (Some(WsChar::Space), Some(WsChar::Tab)) => {
            // Call <label>
            let (id, new_pos) = parse_label(chars, pos + 2)?;
            Ok((Instruction::Call(id), new_pos))
        }
        (Some(WsChar::Space), Some(WsChar::Lf)) => {
            // Jump <label>
            let (id, new_pos) = parse_label(chars, pos + 2)?;
            Ok((Instruction::Jump(id), new_pos))
        }
        (Some(WsChar::Tab), Some(WsChar::Space)) => {
            // JumpIfZero <label>
            let (id, new_pos) = parse_label(chars, pos + 2)?;
            Ok((Instruction::JumpIfZero(id), new_pos))
        }
        (Some(WsChar::Tab), Some(WsChar::Tab)) => {
            // JumpIfNegative <label>
            let (id, new_pos) = parse_label(chars, pos + 2)?;
            Ok((Instruction::JumpIfNegative(id), new_pos))
        }
        (Some(WsChar::Tab), Some(WsChar::Lf)) => {
            // Return
            Ok((Instruction::Return, pos + 2))
        }
        (Some(WsChar::Lf), Some(WsChar::Lf)) => {
            // Exit
            Ok((Instruction::Exit, pos + 2))
        }
        _ => Err(ParseError::InvalidCommand {
            position: pos,
            imp: "Flow".into(),
        }),
    }
}

/// 数値リテラルをパース
///
/// フォーマット: [符号][ビット列][LF]
/// 符号: Space = 正, Tab = 負
/// ビット: Space = 0, Tab = 1 (MSB first)
/// 終端: LF
fn parse_number(chars: &[WsChar], pos: usize) -> Result<(WsNumber, usize), ParseError> {
    // 1. 符号を読む
    let (negative, mut current) = match chars.get(pos) {
        Some(WsChar::Space) => (false, pos + 1),
        Some(WsChar::Tab) => (true, pos + 1),
        Some(WsChar::Lf) => return Ok((WsNumber(0), pos + 1)), // 符号の直後に LF = 0
        None => {
            return Err(ParseError::UnexpectedEof {
                context: "number sign".into(),
            })
        }
    };

    // 2. ビット列を読む（LF まで）
    let mut value: i64 = 0;
    loop {
        match chars.get(current) {
            Some(WsChar::Space) => {
                value = value * 2;
                current += 1;
            }
            Some(WsChar::Tab) => {
                value = value * 2 + 1;
                current += 1;
            }
            Some(WsChar::Lf) => {
                current += 1;
                break;
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    context: "number bits".into(),
                })
            }
        }
    }

    if negative {
        value = -value;
    }
    Ok((WsNumber(value), current))
}

/// ラベルリテラルをパース
///
/// フォーマット: [Space/Tab のシーケンス][LF]
/// ラベルの値は数値と同じエンコーディングだが、符号なし
fn parse_label(chars: &[WsChar], pos: usize) -> Result<(LabelId, usize), ParseError> {
    let (number, new_pos) = parse_number(chars, pos)?;
    Ok((LabelId(number.0.unsigned_abs() as u32), new_pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_push() {
        // Push(1) の正しいエンコーディングをテスト
        // WsNumber(1).encode() = Space Tab LF (符号+, ビット1, 終端)
        // Push = Space Space <num>
        // 合計: Space Space Space Tab LF = "   \t\n"
        let result = parse("   \t\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Instruction::Push(WsNumber(1)));
    }

    #[test]
    fn test_parse_add() {
        // "\t   " = Add
        let result = parse("\t   ").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Instruction::Add);
    }

    #[test]
    fn test_parse_number_zero() {
        // Space LF = 0
        let chars = vec![WsChar::Space, WsChar::Lf];
        let (num, pos) = parse_number(&chars, 0).unwrap();
        assert_eq!(num, WsNumber(0));
        assert_eq!(pos, 2);
    }

    #[test]
    fn test_parse_number_positive() {
        // Space Tab Space LF = +2 (binary: 10)
        let chars = vec![WsChar::Space, WsChar::Tab, WsChar::Space, WsChar::Lf];
        let (num, pos) = parse_number(&chars, 0).unwrap();
        assert_eq!(num, WsNumber(2));
        assert_eq!(pos, 4);
    }

    #[test]
    fn test_parse_number_negative() {
        // Tab Tab Space LF = -2
        let chars = vec![WsChar::Tab, WsChar::Tab, WsChar::Space, WsChar::Lf];
        let (num, pos) = parse_number(&chars, 0).unwrap();
        assert_eq!(num, WsNumber(-2));
        assert_eq!(pos, 4);
    }

    #[test]
    fn test_parse_exit() {
        // "\n\n\n" = Exit
        let result = parse("\n\n\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Instruction::Exit);
    }

    #[test]
    fn test_parse_multiple_instructions() {
        // Push(1), Push(2), Add, Exit
        // Push(1) = Space Space Space Tab LF
        // Push(2) = Space Space Space Tab Space LF (binary: 10)
        // Add = Tab Space Space Space
        // Exit = LF LF LF
        let result = parse("   \t\n   \t \n\t   \n\n\n").unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Instruction::Push(WsNumber(1)));
        assert_eq!(result[1], Instruction::Push(WsNumber(2)));
        assert_eq!(result[2], Instruction::Add);
        assert_eq!(result[3], Instruction::Exit);
    }

    #[test]
    fn test_parse_ignores_other_chars() {
        // "a b   c\td\ne" should be parsed as "   \t\n" = Push(1)
        let result = parse("a b   c\td\ne").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Instruction::Push(WsNumber(1)));
    }

    #[test]
    fn test_roundtrip() {
        // compiler_ws でエンコード → parser でデコード → 一致確認
        use crate::compiler_ws::program::WsProgram;

        let original = vec![
            Instruction::Push(WsNumber(42)),
            Instruction::Push(WsNumber(10)),
            Instruction::Add,
            Instruction::Exit,
        ];

        let mut prog = WsProgram::new();
        for inst in &original {
            prog.push(inst.clone());
        }
        let ws_text = prog.to_whitespace();
        let parsed = parse(&ws_text).unwrap();
        assert_eq!(parsed, original);
    }
}
