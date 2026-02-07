# モジュール詳細設計

## src/whitespace/mod.rs

公開 API のエントリポイント。

```rust
//! # Whitespace モジュール
//!
//! Whitespace 言語のパーサとインタプリタを提供する。
//! 明示的スタックマシンとして実装されており、中断・再開が可能。

mod parser;
mod interpreter;

// compiler_ws から命令型を re-export
pub use crate::compiler_ws::instruction::Instruction;
pub use crate::compiler_ws::types::{WsNumber, LabelId, WsChar};

// パーサ
pub use parser::{parse, ParseError};

// インタプリタ
pub use interpreter::{WhitespaceVM, StepResult, RuntimeError};
```

## src/whitespace/parser.rs

### 責務

Whitespace テキスト（Space / Tab / LF のシーケンス）を `Vec<Instruction>` にパースする。

### 型定義

```rust
/// パースエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// 不正な IMP（命令修飾パラメータ）
    InvalidImp { position: usize },
    /// 不正な命令コマンド部分
    InvalidCommand { position: usize, imp: String },
    /// 予期しないファイル終端
    UnexpectedEof { context: String },
    /// 数値リテラルのパースエラー
    InvalidNumber { position: usize },
    /// ラベルリテラルのパースエラー
    InvalidLabel { position: usize },
}
```

### 内部構造

パーサは以下の2段階で処理する:

1. **文字フィルタリング**: 入力文字列から Space / Tab / LF のみを抽出し `Vec<WsChar>` を作成
2. **命令デコード**: IMP プレフィックスに基づくツリーウォークで命令を1つずつデコード

```
入力: " \t \n\t  \n\n\n"
         ↓ フィルタリング
WsChar列: [S, T, S, LF, T, S, S, LF, LF, LF]
         ↓ デコード
命令列:   [Push(1), OutputNumber, Exit]
```

### 数値パース

```rust
/// 数値リテラルをパース
/// 
/// フォーマット: [符号][ビット列][LF]
/// 符号: Space = 正, Tab = 負
/// ビット: Space = 0, Tab = 1 (MSB first)
/// 終端: LF
fn parse_number(chars: &[WsChar], pos: usize) -> Result<(WsNumber, usize), ParseError> {
    // 1. 符号を読む
    let (negative, pos) = match chars.get(pos) {
        Some(WsChar::Space) => (false, pos + 1),
        Some(WsChar::Tab) => (true, pos + 1),
        Some(WsChar::Lf) => return Ok((WsNumber(0), pos + 1)), // 符号の直後に LF = 0
        None => return Err(ParseError::UnexpectedEof { context: "number sign".into() }),
    };

    // 2. ビット列を読む（LF まで）
    let mut value: i64 = 0;
    let mut current = pos;
    loop {
        match chars.get(current) {
            Some(WsChar::Space) => { value = value * 2; current += 1; }
            Some(WsChar::Tab) => { value = value * 2 + 1; current += 1; }
            Some(WsChar::Lf) => { current += 1; break; }
            None => return Err(ParseError::UnexpectedEof { context: "number bits".into() }),
        }
    }

    if negative { value = -value; }
    Ok((WsNumber(value), current))
}
```

### ラベルパース

```rust
/// ラベルリテラルをパース
///
/// フォーマット: [Space/Tab のシーケンス][LF]
/// ラベルの値は数値と同じエンコーディングだが、符号なし
/// ※ ただし LabelId への変換のため、ビット列を整数値として解釈
fn parse_label(chars: &[WsChar], pos: usize) -> Result<(LabelId, usize), ParseError> {
    let (number, new_pos) = parse_number(chars, pos)?;
    Ok((LabelId(number.0 as u32), new_pos))
}
```

### IMP ごとのパース関数

| 関数 | IMP プレフィックス | パースする命令 |
|------|-------------------|---------------|
| `parse_stack_op` | `[S]` | Push, Dup, Copy, Swap, Discard, Slide |
| `parse_arithmetic` | `[T][S]` | Add, Sub, Mul, Div, Mod |
| `parse_heap` | `[T][T]` | Store, Retrieve |
| `parse_io` | `[T][LF]` | OutputChar, OutputNumber, InputChar, InputNumber |
| `parse_flow` | `[LF]` | Label, Call, Jump, JumpIfZero, JumpIfNegative, Return, Exit |

## src/whitespace/interpreter.rs

### 責務

Whitespace 命令列を実行するスタックマシン。全ての実行状態を明示的に保持し、中断・再開可能。

### WhitespaceVM 構造体

```rust
pub struct WhitespaceVM {
    // プログラム
    instructions: Vec<Instruction>,
    labels: HashMap<i64, usize>,

    // 実行状態
    pc: usize,
    data_stack: Vec<i64>,
    call_stack: Vec<usize>,
    heap: HashMap<i64, i64>,

    // I/O
    stdin: Box<dyn BufRead>,
    stdout: Box<dyn Write>,

    // メトリクス
    total_steps: usize,

    // 拡張 API
    pub traced: BTreeMap<i64, i64>,

    // 状態フラグ
    completed: bool,
}
```

### 内部enum

```rust
/// 1命令の実行結果（内部使用）
enum ExecuteResult {
    /// 次の命令へ進む
    Continue,
    /// プログラム終了 (Exit 命令)
    Exit,
    /// 実行時エラー
    Error(RuntimeError),
}
```

### ラベル収集

VM 初期化時に、命令列を走査して全ての `Label(id)` の位置を HashMap に記録する。

```rust
fn collect_labels(instructions: &[Instruction]) -> HashMap<i64, usize> {
    let mut labels = HashMap::new();
    for (i, inst) in instructions.iter().enumerate() {
        if let Instruction::Label(id) = inst {
            labels.insert(id.to_ws_value(), i);
        }
    }
    labels
}
```

実行時のラベル解決は O(1)。

### I/O の読み取り実装

Whitespace の I/O 命令は nospace インタプリタの `Environment` にある read_int / read_char と同じ動作を提供する。

```rust
/// 標準入力から1文字を読み取り、その文字コードを返す
fn read_char(&mut self) -> Result<i64, RuntimeError> {
    let mut buf = [0u8; 1];
    match self.stdin.read(&mut buf) {
        Ok(1) => Ok(buf[0] as i64),
        Ok(_) => Ok(0), // EOF
        Err(e) => Err(RuntimeError::IoError(e.to_string())),
    }
}

/// 標準入力から整数を読み取る
fn read_number(&mut self) -> Result<i64, RuntimeError> {
    let mut line = String::new();
    self.stdin.read_line(&mut line)
        .map_err(|e| RuntimeError::IoError(e.to_string()))?;
    line.trim().parse::<i64>()
        .map_err(|e| RuntimeError::IoError(e.to_string()))
}
```

### 拡張 API マッピング

[whitespace-runtime.md](../../architecture/whitespace-runtime.md) で定義された仕様に基づく:

| ヒープアドレス | 書き込み動作 | 対応する nospace 関数 |
|--------------|-------------|---------------------|
| `-1` | `traced[val] += 1` | `__trace(n)` |
| `-2` | `val == 0` → エラー | `__assert(n)` |
| `-3` | `val != 0` → エラー | `__assert_not(n)` |

負アドレスへの読み取り (Retrieve) は通常のヒープと同じ扱い（0 を返す）。

## src/compiler_ws/program.rs への変更

`WsProgram` に命令列の取り出しメソッドを追加:

```rust
impl WsProgram {
    // 既存メソッド...

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
```

## テスト戦略

### Unit テスト（各モジュール内）

#### parser テスト

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_push() {
        // "  \t\n" = Push(1)
        let result = parse("  \t\n").unwrap();
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
    fn test_roundtrip() {
        // compiler_ws でエンコード → parser でデコード → 一致確認
        let original = vec![
            Instruction::Push(WsNumber(42)),
            Instruction::Push(WsNumber(10)),
            Instruction::Add,
            Instruction::Exit,
        ];
        let ws_text = WsProgram::from(original.clone()).to_whitespace();
        let parsed = parse(&ws_text).unwrap();
        assert_eq!(parsed, original);
    }
}
```

#### interpreter テスト

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_add() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(2)),
            Instruction::Push(WsNumber(3)),
            Instruction::Add,
            Instruction::Exit,
        ]);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[5]);
    }

    #[test]
    fn test_suspension() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(1)),
            Instruction::Push(WsNumber(2)),
            Instruction::Push(WsNumber(3)),
            Instruction::Add,
            Instruction::Add,
            Instruction::Exit,
        ]);
        // budget=2 で中断
        let result = vm.step(2);
        assert_eq!(result, StepResult::Suspended);
        assert_eq!(vm.data_stack(), &[1, 2]); // 2命令分のみ実行

        // 残りを実行
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[6]);
    }

    #[test]
    fn test_subroutine_call() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            // 0: jump to label 1
            Instruction::Jump(LabelId(1)),
            // 1: subroutine at label 2
            Instruction::Label(LabelId(2)),
            Instruction::Push(WsNumber(42)),
            Instruction::Return,
            // 4: main code at label 1
            Instruction::Label(LabelId(1)),
            Instruction::Call(LabelId(2)),
            Instruction::Exit,
        ]);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[42]);
    }

    #[test]
    fn test_trace_extension() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            // __trace(7): push -1, push 7, store
            Instruction::Push(WsNumber(-1)),
            Instruction::Push(WsNumber(7)),
            Instruction::Store,
            Instruction::Exit,
        ]);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.traced.get(&7), Some(&1));
    }

    #[test]
    fn test_heap_store_retrieve() {
        let mut vm = WhitespaceVM::from_instructions(vec![
            Instruction::Push(WsNumber(100)),  // addr
            Instruction::Push(WsNumber(42)),   // value
            Instruction::Store,
            Instruction::Push(WsNumber(100)),  // addr
            Instruction::Retrieve,
            Instruction::Exit,
        ]);
        let result = vm.run(100);
        assert_eq!(result, StepResult::Complete);
        assert_eq!(vm.data_stack(), &[42]);
    }
}
```

### 統合テスト（wsc との結果比較）

Phase 3 で、既存の `resources/tests/` のテストケースを以下の2経路で実行し、結果が一致することを検証する:

```
nospace ソース → compile → WsProgram → WhitespaceVM → stdout_a
                                       └─→ wsc (external)   → stdout_b
assert_eq!(stdout_a, stdout_b)
```

既存の `tests/common/mod.rs` の `run_whitespace()` (wsc 実行) と、新規の `run_whitespace_vm()` (自前 VM 実行) を併用する。

`test-manifest.yaml` に `whitespace_vm` ターゲットを追加し、`build.rs` で比較テストを自動生成する。
wsc が利用不可の環境では VM 単体のテストのみ実行し、wsc 比較はスキップする。
