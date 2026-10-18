//! Whitespace パーサ
//!
//! Whitespace テキスト（Space / Tab / LF のシーケンス）を `Vec<Instruction>` にパースする

use super::label_interner::LabelInterner;
use crate::base::ws_types::{Instruction, LabelId, WsChar, WsNumber};

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
    let mut labels = LabelInterner::new();

    while pos < chars.len() {
        let (inst, new_pos) = parse_instruction(&chars, pos, &mut labels)?;
        instructions.push(inst);
        pos = new_pos;
    }

    Ok(instructions)
}

/// 1命令をパース
fn parse_instruction<'a>(
    chars: &'a [WsChar],
    pos: usize,
    labels: &mut LabelInterner<'a>,
) -> Result<(Instruction, usize), ParseError> {
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
        Some(WsChar::Lf) => parse_flow(chars, pos + 1, labels),
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
fn parse_flow<'a>(
    chars: &'a [WsChar],
    pos: usize,
    labels: &mut LabelInterner<'a>,
) -> Result<(Instruction, usize), ParseError> {
    match (chars.get(pos), chars.get(pos + 1)) {
        (Some(WsChar::Space), Some(WsChar::Space)) => {
            // Label <label>
            let (id, new_pos) = parse_label(chars, pos + 2, labels)?;
            Ok((Instruction::Label(id), new_pos))
        }
        (Some(WsChar::Space), Some(WsChar::Tab)) => {
            // Call <label>
            let (id, new_pos) = parse_label(chars, pos + 2, labels)?;
            Ok((Instruction::Call(id), new_pos))
        }
        (Some(WsChar::Space), Some(WsChar::Lf)) => {
            // Jump <label>
            let (id, new_pos) = parse_label(chars, pos + 2, labels)?;
            Ok((Instruction::Jump(id), new_pos))
        }
        (Some(WsChar::Tab), Some(WsChar::Space)) => {
            // JumpIfZero <label>
            let (id, new_pos) = parse_label(chars, pos + 2, labels)?;
            Ok((Instruction::JumpIfZero(id), new_pos))
        }
        (Some(WsChar::Tab), Some(WsChar::Tab)) => {
            // JumpIfNegative <label>
            let (id, new_pos) = parse_label(chars, pos + 2, labels)?;
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
///
/// 64 ビット符号付き整数の範囲外の値は `NumberOverflow` を返す。
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
    // バグ修正: 従来は正方向に未チェックで累積していたため、64 ビットを超えるビット列で
    // panic（debug）/ wrap（release）し、絶対値 2^63 の i64::MIN も読めなかった。
    // 解決策: 負方向に checked 演算で累積し、正の数のときだけ最後に checked_neg する。
    let overflow = || ParseError::NumberOverflow { position: pos };
    let mut value: i64 = 0; // 常に 0 以下（絶対値の符号反転）
    loop {
        let bit = match chars.get(current) {
            Some(WsChar::Space) => 0,
            Some(WsChar::Tab) => 1,
            Some(WsChar::Lf) => {
                current += 1;
                break;
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    context: "number bits".into(),
                })
            }
        };
        value = value
            .checked_mul(2)
            .and_then(|v| v.checked_sub(bit))
            .ok_or_else(overflow)?;
        current += 1;
    }

    let value = if negative {
        value
    } else {
        value.checked_neg().ok_or_else(overflow)?
    };
    Ok((WsNumber(value), current))
}

/// ラベルリテラルをパース
///
/// フォーマット: [Space/Tab のシーケンス][LF]（符号なし、長さ任意、空列も可）
// バグ修正: 従来は数値としてパースして unsigned_abs() as u32 に変換していたため、
// 先頭文字が符号として捨てられ [S,T] と [T,T] が同じラベルになり、
// 32 ビット超のラベルも衝突していた。
// 解決策: ビット列そのものを LabelInterner で連番 ID に写像する。
fn parse_label<'a>(
    chars: &'a [WsChar],
    pos: usize,
    labels: &mut LabelInterner<'a>,
) -> Result<(LabelId, usize), ParseError> {
    let len = chars
        .get(pos..)
        .unwrap_or_default()
        .iter()
        .position(|c| *c == WsChar::Lf)
        .ok_or_else(|| ParseError::UnexpectedEof {
            context: "label".into(),
        })?;
    let bits = &chars[pos..pos + len];
    let id = labels
        .intern(bits)
        .ok_or(ParseError::InvalidLabel { position: pos })?;
    Ok((id, pos + len + 1))
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
