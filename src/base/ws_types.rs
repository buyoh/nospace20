//! Whitespace 言語の共有基本型
//!
//! `compiler_ws` と `whitespace` の両モジュールから参照される型を定義する。
//! これにより `whitespace` が `compiler_ws` の内部型に直接依存することを防ぐ。

// ===== WsChar =====

/// Whitespace の基本文字
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsChar {
    Space, // SP (0x20)
    Tab,   // TB (0x09)
    Lf,    // LF (0x0A)
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

// ===== WsNumber =====

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

// ===== LabelId =====

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

// ===== HeapAddress =====

/// ヒープアドレス
///
/// Whitespace のヒープ上のアドレスを表す。
/// Debug 出力で実際の数値も確認可能。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeapAddress(pub i64);

impl HeapAddress {
    #[allow(dead_code)]
    pub const fn new(addr: i64) -> Self {
        Self(addr)
    }

    /// アドレス値を取得（Whitespace 命令生成用）
    #[allow(dead_code)]
    pub fn value(&self) -> i64 {
        self.0
    }

    /// オフセットを加算した新しいアドレスを返す
    #[allow(dead_code)]
    pub fn offset(&self, n: i64) -> Self {
        Self(self.0 + n)
    }
}

impl std::fmt::Debug for HeapAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HeapAddr({})", self.0)
    }
}

impl std::fmt::Display for HeapAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

// ===== Instruction =====

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

// ===== WsProgram =====

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
        // encoder::to_string と同等のロジックをインライン化
        self.instructions
            .iter()
            .flat_map(|inst| inst.encode())
            .map(|c| c.to_char())
            .collect()
    }

    /// デバッグ用のニーモニック表現
    pub fn to_debug_string(&self) -> String {
        self.instructions
            .iter()
            .map(|inst| {
                if matches!(inst, Instruction::Label(_)) {
                    inst.to_mnemonic()
                } else {
                    format!("    {}", inst.to_mnemonic())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 命令数を取得
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// 空かどうかを判定
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// 命令列を消費して Vec<Instruction> を返す
    /// WhitespaceVM へ渡す際に使用
    #[allow(dead_code)]
    pub fn into_instructions(self) -> Vec<Instruction> {
        self.instructions
    }

    /// Vec<Instruction> から WsProgram を生成
    pub fn from_instructions(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    /// 命令列への参照を返す
    #[allow(dead_code)]
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
}

impl Default for WsProgram {
    fn default() -> Self {
        Self::new()
    }
}

// ===== テスト =====

#[cfg(test)]
#[path = "ws_types_tests.rs"]
mod tests;
