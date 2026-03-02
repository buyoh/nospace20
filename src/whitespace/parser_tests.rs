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
