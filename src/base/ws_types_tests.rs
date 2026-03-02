use super::*;

// -- WsNumber --

#[test]
fn test_encode_number_positive() {
    let n = WsNumber(5);
    // 5 = 101 (binary)
    // [Space (positive), Tab, Space, Tab, Lf]
    assert_eq!(
        n.encode(),
        vec![
            WsChar::Space,
            WsChar::Tab,
            WsChar::Space,
            WsChar::Tab,
            WsChar::Lf
        ]
    );
}

#[test]
fn test_encode_number_zero() {
    let n = WsNumber(0);
    assert_eq!(n.encode(), vec![WsChar::Space, WsChar::Lf]);
}

#[test]
fn test_encode_number_negative() {
    let n = WsNumber(-1);
    // -1: 絶対値=1=1(binary) → [Tab, Tab, Lf]
    assert_eq!(n.encode(), vec![WsChar::Tab, WsChar::Tab, WsChar::Lf]);
}

// -- LabelId --

#[test]
fn test_label_offset() {
    let l1 = LabelId(16);
    let l2 = l1.offset(5);
    assert_eq!(l2.0, 21);
}

// -- HeapAddress --

#[test]
fn test_heap_address_offset() {
    let addr = HeapAddress::new(100);
    let addr2 = addr.offset(50);
    assert_eq!(addr2.value(), 150);
}

// -- Instruction --

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

// -- WsProgram --

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
    // Exit = LF LF LF
    assert_eq!(prog.to_whitespace(), "\n\n\n");
}
