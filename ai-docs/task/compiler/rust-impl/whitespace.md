# Whitespace 命令定義・エンコーダ

## 基本型

### WsChar - Whitespace 基本文字

```rust
/// Whitespace の基本文字
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsChar {
    Space,  // SP (0x20)
    Tab,    // TB (0x09)
    Lf,     // LF (0x0A)
}

impl WsChar {
    pub fn to_char(&self) -> char {
        match self {
            WsChar::Space => ' ',
            WsChar::Tab => '\t',
            WsChar::Lf => '\n',
        }
    }
}
```

### WsNumber - 数値パラメータ

```rust
/// 数値パラメータ (符号付き整数)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsNumber(pub i64);

impl WsNumber {
    /// Whitespace 形式にエンコード
    pub fn encode(&self) -> Vec<WsChar> {
        use WsChar::*;
        
        let mut result = Vec::new();
        
        // 符号
        if self.0 < 0 {
            result.push(Tab);
        } else {
            result.push(Space);
        }
        
        // 絶対値をビット列に (MSB first)
        let abs_val = self.0.unsigned_abs();
        if abs_val == 0 {
            // 0 はビットなし
        } else {
            let bits = 64 - abs_val.leading_zeros();
            for i in (0..bits).rev() {
                if (abs_val >> i) & 1 == 1 {
                    result.push(Tab);
                } else {
                    result.push(Space);
                }
            }
        }
        
        // 終端
        result.push(Lf);
        result
    }
}
```

### LabelId - ラベル識別子

```rust
/// ラベル識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelId(pub u32);

impl LabelId {
    /// Whitespace 出力用の値
    pub fn to_ws_value(&self) -> i64 {
        self.0 as i64
    }
    
    /// オフセットを加算
    pub fn offset(&self, n: u32) -> LabelId {
        LabelId(self.0 + n)
    }
}
```

## Instruction - 命令定義

```rust
/// Whitespace 命令
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // === スタック操作 (IMP: SP) ===
    Push(WsNumber),      // SP SP <number>
    Duplicate,           // SP LF SP
    Copy(WsNumber),      // SP TB SP <n>
    Swap,                // SP LF TB
    Discard,             // SP LF LF
    
    // === 算術演算 (IMP: TB SP) ===
    Add,                 // TB SP SP SP
    Sub,                 // TB SP SP TB
    Mul,                 // TB SP SP LF
    Div,                 // TB SP TB SP
    Mod,                 // TB SP TB TB
    
    // === ヒープアクセス (IMP: TB TB) ===
    Store,               // TB TB SP
    Retrieve,            // TB TB TB
    
    // === フロー制御 (IMP: LF) ===
    Label(LabelId),      // LF SP SP <label>
    Call(LabelId),       // LF SP TB <label>
    Jump(LabelId),       // LF SP LF <label>
    JumpIfZero(LabelId), // LF TB SP <label>
    JumpIfNegative(LabelId), // LF TB TB <label>
    Return,              // LF TB LF
    Exit,                // LF LF LF
    
    // === I/O (IMP: TB LF) ===
    OutputChar,          // TB LF SP SP
    OutputNumber,        // TB LF SP TB
    InputChar,           // TB LF TB SP
    InputNumber,         // TB LF TB TB
}
```

## 命令エンコーダ

```rust
impl Instruction {
    pub fn encode(&self) -> Vec<WsChar> {
        use WsChar::*;
        use Instruction::*;
        
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
}
```

## WsProgram - プログラム構造

```rust
/// 生成された Whitespace プログラム
#[derive(Debug, Default)]
pub struct WsProgram {
    instructions: Vec<Instruction>,
}

impl WsProgram {
    pub fn new() -> Self {
        Self { instructions: Vec::new() }
    }
    
    /// 命令を追加
    pub fn push(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }
    
    /// 複数の命令を追加
    pub fn extend(&mut self, insts: impl IntoIterator<Item = Instruction>) {
        self.instructions.extend(insts);
    }
    
    /// 他のプログラムを結合
    pub fn append(&mut self, mut other: WsProgram) {
        self.instructions.append(&mut other.instructions);
    }
    
    /// Whitespace 文字列にエンコード
    pub fn to_whitespace(&self) -> String {
        self.instructions
            .iter()
            .flat_map(|inst| inst.encode())
            .map(|c| c.to_char())
            .collect()
    }
    
    /// デバッグ用の可読形式に変換
    pub fn to_debug_string(&self) -> String {
        self.instructions
            .iter()
            .map(|inst| format!("{:?}", inst))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

## テスト例

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_number_positive() {
        // 5 = 101 (binary)
        let n = WsNumber(5);
        assert_eq!(
            n.encode(),
            vec![WsChar::Space, WsChar::Tab, WsChar::Space, WsChar::Tab, WsChar::Lf]
        );
    }
    
    #[test]
    fn test_encode_number_zero() {
        let n = WsNumber(0);
        assert_eq!(n.encode(), vec![WsChar::Space, WsChar::Lf]);
    }
    
    #[test]
    fn test_encode_number_negative() {
        // -3 = 11 (binary)
        let n = WsNumber(-3);
        assert_eq!(
            n.encode(),
            vec![WsChar::Tab, WsChar::Tab, WsChar::Tab, WsChar::Lf]
        );
    }
    
    #[test]
    fn test_encode_add() {
        let inst = Instruction::Add;
        assert_eq!(
            inst.encode(),
            vec![WsChar::Tab, WsChar::Space, WsChar::Space, WsChar::Space]
        );
    }
}
```
