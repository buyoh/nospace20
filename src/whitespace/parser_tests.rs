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

// ===== 数値の境界値 =====

/// 符号 + ビット列 + LF の WsChar 列を作る
fn number_chars(negative: bool, bits: &[WsChar]) -> Vec<WsChar> {
    let mut v = vec![if negative { WsChar::Tab } else { WsChar::Space }];
    v.extend_from_slice(bits);
    v.push(WsChar::Lf);
    v
}

#[test]
fn test_parse_number_i64_max_roundtrip() {
    let chars = WsNumber(i64::MAX).encode();
    let (num, pos) = parse_number(&chars, 0).unwrap();
    assert_eq!(num, WsNumber(i64::MAX));
    assert_eq!(pos, chars.len());
}

#[test]
fn test_parse_number_i64_min_roundtrip() {
    let chars = WsNumber(i64::MIN).encode();
    let (num, pos) = parse_number(&chars, 0).unwrap();
    assert_eq!(num, WsNumber(i64::MIN));
    assert_eq!(pos, chars.len());
}

#[test]
fn test_parse_number_positive_2_pow_63_overflows() {
    // +2^63 = i64::MAX + 1
    let mut bits = vec![WsChar::Tab];
    bits.extend(std::iter::repeat(WsChar::Space).take(63));
    let chars = number_chars(false, &bits);
    assert_eq!(
        parse_number(&chars, 0),
        Err(ParseError::NumberOverflow { position: 0 })
    );
}

#[test]
fn test_parse_number_65_bits_overflows() {
    let bits = vec![WsChar::Tab; 65];
    for negative in [false, true] {
        let chars = number_chars(negative, &bits);
        assert_eq!(
            parse_number(&chars, 0),
            Err(ParseError::NumberOverflow { position: 0 }),
            "negative={}",
            negative
        );
    }
}

#[test]
fn test_parse_number_long_leading_zeros() {
    let mut bits = vec![WsChar::Space; 100];
    bits.push(WsChar::Tab);
    let chars = number_chars(false, &bits);
    let (num, _) = parse_number(&chars, 0).unwrap();
    assert_eq!(num, WsNumber(1));
}

#[test]
fn test_parse_push_overflow_is_error() {
    // Push + 65 ビット
    let src = format!("   {}\n", "\t".repeat(65));
    assert_eq!(parse(&src), Err(ParseError::NumberOverflow { position: 2 }));
}

#[test]
fn test_roundtrip_push_i64_min() {
    let original = vec![Instruction::Push(WsNumber(i64::MIN)), Instruction::Exit];
    let text: String = original
        .iter()
        .flat_map(|i| i.encode())
        .map(|c| c.to_char())
        .collect();
    assert_eq!(parse(&text).unwrap(), original);
}

// ===== ラベル =====

/// LF S S <label> LF = Label
fn label_def(bits: &str) -> String {
    format!("\n  {}\n", bits)
}

fn label_of(inst: &Instruction) -> LabelId {
    match inst {
        Instruction::Label(id)
        | Instruction::Call(id)
        | Instruction::Jump(id)
        | Instruction::JumpIfZero(id)
        | Instruction::JumpIfNegative(id) => *id,
        other => panic!("not a label instruction: {:?}", other),
    }
}

#[test]
fn test_parse_labels_distinguished_by_bits() {
    // 数値として解釈するといずれも 1 になる [S,T] [T,T] [S,S,T]、および空ラベル
    let src = [" \t", "\t\t", "  \t", ""]
        .iter()
        .map(|b| label_def(b))
        .collect::<String>();
    let parsed = parse(&src).unwrap();
    let ids: Vec<LabelId> = parsed.iter().map(label_of).collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "labels {} and {} collided", i, j);
        }
    }
}

#[test]
fn test_parse_label_forward_reference() {
    // Call [T,S] → Label [T,S]
    let src = format!("\n \t\t \n{}", label_def("\t "));
    let parsed = parse(&src).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(label_of(&parsed[0]), label_of(&parsed[1]));
}

#[test]
fn test_parse_long_label() {
    let src = label_def(&"\t".repeat(100));
    let parsed = parse(&src).unwrap();
    assert!(matches!(parsed[0], Instruction::Label(_)));
}

#[test]
fn test_parse_label_eof() {
    assert!(matches!(
        parse("\n  \t"),
        Err(ParseError::UnexpectedEof { .. })
    ));
}

#[test]
fn test_parse_slide() {
    // Slide = Space Tab LF <num>、WsNumber(1) = Space Tab LF
    let result = parse(" \t\n \t\n").unwrap();
    assert_eq!(result, vec![Instruction::Slide(WsNumber(1))]);
}
