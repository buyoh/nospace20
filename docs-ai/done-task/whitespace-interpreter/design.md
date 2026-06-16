# Whitespace インタプリタ 全体設計

## モジュール構造

```
src/
├── bin/
│   ├── nospace20.rs        # 既存: nospace CLI
│   └── whitespace20.rs     # 新規: Whitespace インタプリタ CLI
├── whitespace/
│   ├── mod.rs              # 公開 API (WhitespaceVM, StepResult, parse)
│   ├── parser.rs           # Whitespace テキスト → Vec<Instruction> パーサ
│   └── interpreter.rs      # 実行エンジン (WhitespaceVM)
├── compiler_ws/            # 既存: nospace → Whitespace コンパイラ
│   ├── instruction.rs      # 命令 enum (Instruction) ← 共有元
│   └── ...
└── lib.rs                  # mod whitespace 追加、公開 API 追加
```

### compiler_ws との型共有

`compiler_ws::instruction::Instruction` を Whitespace インタプリタでも使用する。

**方式: compiler_ws の型を whitespace モジュール経由で re-export**

```rust
// src/whitespace/mod.rs
pub use crate::compiler_ws::instruction::Instruction;
pub use crate::compiler_ws::types::{WsNumber, LabelId, WsChar};
```

**理由**:
- コンパイラ出力 `WsProgram` の `Vec<Instruction>` をそのままインタプリタに渡せる
- 型定義の重複を避ける
- 将来的に shared module に移動する際の影響が最小

## 公開 API

### 型定義

```rust
// src/whitespace/interpreter.rs

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};

/// VM の実行結果
#[derive(Debug, PartialEq)]
pub enum StepResult {
    /// 実行継続中（バジェット消費で中断）
    Suspended,
    /// 正常終了（Exit 命令到達）
    Complete,
    /// 実行時エラー
    Error(RuntimeError),
}

/// 実行時エラー
#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    /// スタックアンダーフロー
    StackUnderflow,
    /// ゼロ除算
    DivisionByZero,
    /// 未定義ラベルへのジャンプ
    UndefinedLabel(i64),
    /// ヒープの未初期化アドレスへのアクセス
    UninitializedHeap(i64),
    /// コールスタックアンダーフロー（ret 命令でコールスタックが空）
    CallStackUnderflow,
    /// PC が命令列の範囲外
    ProgramCounterOutOfBounds,
    /// I/O エラー
    IoError(String),
    /// アサーション失敗（拡張 API）
    AssertionFailed(i64),
}
```

### WhitespaceVM

```rust
/// Whitespace 仮想マシン
///
/// 明示的スタックマシンとして全ての実行状態を保持する。
/// step() メソッドで指定ステップ数だけ実行し、自動的に中断する。
pub struct WhitespaceVM {
    // === プログラム ===
    /// 命令列
    instructions: Vec<Instruction>,
    /// ラベル → 命令インデックスのマッピング
    labels: HashMap<i64, usize>,

    // === 実行状態 ===
    /// プログラムカウンタ（次に実行する命令のインデックス）
    pc: usize,
    /// データスタック
    data_stack: Vec<i64>,
    /// コールスタック（サブルーチン call 時の戻りアドレス）
    call_stack: Vec<usize>,
    /// ヒープメモリ
    heap: HashMap<i64, i64>,

    // === I/O ===
    stdin: Box<dyn BufRead>,
    stdout: Box<dyn Write>,

    // === メトリクス ===
    /// 総実行命令数
    total_steps: usize,

    // === 拡張 API ===
    /// トレース記録（__trace 拡張 API の出力先）
    pub traced: BTreeMap<i64, i64>,

    // === 実行状態フラグ ===
    /// 実行完了済みかどうか
    completed: bool,
}
```

### メソッド一覧

```rust
impl WhitespaceVM {
    // === コンストラクタ ===

    /// Whitespace テキストから VM を構築
    pub fn from_source(source: &str) -> Result<Self, ParseError>;

    /// 命令列から VM を構築（compiler_ws のパイプライン用）
    pub fn from_instructions(instructions: Vec<Instruction>) -> Self;

    /// I/O バッファを指定して構築
    pub fn with_io(
        self,
        stdin: Box<dyn BufRead>,
        stdout: Box<dyn Write>,
    ) -> Self;

    // === 実行 ===

    /// 指定ステップ数だけ実行し、結果を返す
    ///
    /// - budget > 0: 最大 budget 命令を実行
    /// - 途中で Exit/Error に到達した場合は即座に返す
    /// - budget を消費しきった場合は Suspended を返す
    pub fn step(&mut self, budget: usize) -> StepResult;

    /// 完了まで一括実行（最大ステップ制限付き）
    pub fn run(&mut self, max_steps: usize) -> StepResult;

    // === 状態参照 ===

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool;

    /// データスタックの現在の状態
    pub fn data_stack(&self) -> &[i64];

    /// ヒープの現在の状態
    pub fn heap(&self) -> &HashMap<i64, i64>;

    /// 総実行命令数
    pub fn total_steps(&self) -> usize;

    /// stdout の内容を取得（バッファ使用時）
    pub fn flush(&mut self);
}
```

### lib.rs 公開 API

```rust
// src/lib.rs に追加

pub use whitespace::{WhitespaceVM, StepResult, RuntimeError};
pub use whitespace::parser::{parse as parse_whitespace, ParseError as WsParseError};
```

### CLI バイナリ: whitespace20

`src/bin/whitespace20.rs` に新しい CLI バイナリを作成する。

#### コマンドラインインターフェース

```
whitespace20 - A Whitespace language interpreter

Usage: whitespace20 [OPTIONS] <FILE>

Arguments:
  <FILE>  Whitespace source file to execute

Options:
  -i, --input <FILE>    Read stdin from file (default: stdin)
  -o, --output <FILE>   Write stdout to file (default: stdout)
  -m, --max-steps <N>   Maximum execution steps (default: unlimited)
      --debug           Show execution metrics after run
  -h, --help            Print help
  -V, --version         Print version
```

#### 設計

```rust
// src/bin/whitespace20.rs

use clap::Parser;
use nospace20::whitespace::{WhitespaceVM, StepResult};

/// whitespace20 - A Whitespace language interpreter
#[derive(Parser, Debug)]
#[command(name = "whitespace20")]
#[command(version = "0.1.0")]
#[command(about = "A Whitespace language interpreter")]
struct Args {
    /// Whitespace source file to execute
    file: String,

    /// Read stdin from file
    #[arg(short, long, value_name = "FILE")]
    input: Option<String>,

    /// Write stdout to file
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Maximum execution steps (0 = unlimited)
    #[arg(short, long, default_value_t = 0)]
    max_steps: usize,

    /// Show execution metrics after run
    #[arg(long)]
    debug: bool,
}
```

#### 実行フロー

```
1. ファイル読み込み
2. WhitespaceVM::from_source() でパース + VM 初期化
3. stdin/stdout の設定（ファイル指定時はファイル I/O）
4. vm.run(max_steps) で実行
5. 結果に応じて終了コード設定
   - Complete → exit(0)
   - Error → stderr にエラー表示、exit(1)
6. --debug 時: 実行ステップ数、スタックサイズ等を stderr に出力
```

#### wsc との互換性

whitespace20 は wsc と同じ使い方ができるようにする（サブセット）:

| wsc | whitespace20 | 備考 |
|-----|-------------|------|
| `wsc program.ws` | `whitespace20 program.ws` | 基本実行 |
| `wsc program.ws -i input.txt` | `whitespace20 program.ws -i input.txt` | stdin ファイル指定 |
| `wsc program.ws -o output.txt` | `whitespace20 program.ws -o output.txt` | stdout ファイル指定 |

**差異**: whitespace20 は拡張 API（負ヒープアドレス）を解釈する。wsc は標準 Whitespace のみ。

#### Cargo.toml への追加

```toml
# [[bin]] セクションは src/bin/ 配下のファイルから自動検出されるため、
# Cargo.toml への明示的な追加は不要。
# ただし、nospace20 と whitespace20 で共通の依存関係（clap 等）を使用する。
```

## 内部設計

### VM 初期化フロー

```
[入力: Whitespace テキスト or Vec<Instruction>]
    │
    ▼
┌───────────────────┐
│ 1. パース/受け取り   │  → Vec<Instruction>
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ 2. ラベル収集       │  → HashMap<i64, usize>
│   Label(id) 命令の  │     ラベル値 → 命令インデックス
│   位置を記録        │
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ 3. VM 状態初期化    │  pc=0, stack=[], heap={}, call_stack=[]
└───────────────────┘
```

### 実行ループ（step メソッド）

```rust
pub fn step(&mut self, budget: usize) -> StepResult {
    if self.completed {
        return StepResult::Complete;
    }

    for _ in 0..budget {
        if self.pc >= self.instructions.len() {
            return StepResult::Error(RuntimeError::ProgramCounterOutOfBounds);
        }

        match self.execute_instruction() {
            ExecuteResult::Continue => {
                self.total_steps += 1;
            }
            ExecuteResult::Exit => {
                self.completed = true;
                return StepResult::Complete;
            }
            ExecuteResult::Error(e) => {
                return StepResult::Error(e);
            }
        }
    }

    StepResult::Suspended
}
```

**中断のポイント**: ループの反復境界（命令と命令の間）で自然に中断する。
Whitespace はフラットな命令セットなので、1命令の実行は原子的であり、命令の途中での中断は不要。

### 命令ディスパッチ（execute_instruction メソッド）

```rust
/// 1命令を実行する
fn execute_instruction(&mut self) -> ExecuteResult {
    // 注意: self.instructions[self.pc] を clone せず参照で処理
    // Instruction が Clone を derive しているため、必要な場合のみ clone
    let pc = self.pc;
    
    match &self.instructions[pc] {
        // === スタック操作 ===
        Instruction::Push(n) => {
            self.data_stack.push(n.0);
            self.pc += 1;
        }
        Instruction::Duplicate => {
            let val = self.stack_top()?;
            self.data_stack.push(val);
            self.pc += 1;
        }
        Instruction::Copy(n) => {
            let idx = self.data_stack.len().checked_sub(1 + n.0 as usize)
                .ok_or(RuntimeError::StackUnderflow)?;
            let val = self.data_stack[idx];
            self.data_stack.push(val);
            self.pc += 1;
        }
        Instruction::Swap => {
            let len = self.data_stack.len();
            if len < 2 { return ExecuteResult::Error(RuntimeError::StackUnderflow); }
            self.data_stack.swap(len - 1, len - 2);
            self.pc += 1;
        }
        Instruction::Discard => {
            self.stack_pop()?;
            self.pc += 1;
        }

        // === 算術演算 ===
        Instruction::Add => { self.binary_op(|a, b| Ok(a + b))?; }
        Instruction::Sub => { self.binary_op(|a, b| Ok(a - b))?; }
        Instruction::Mul => { self.binary_op(|a, b| Ok(a * b))?; }
        Instruction::Div => {
            self.binary_op(|a, b| {
                if b == 0 { Err(RuntimeError::DivisionByZero) } else { Ok(a / b) }
            })?;
        }
        Instruction::Mod => {
            self.binary_op(|a, b| {
                if b == 0 { Err(RuntimeError::DivisionByZero) } else { Ok(a % b) }
            })?;
        }

        // === ヒープアクセス ===
        Instruction::Store => {
            let val = self.stack_pop()?;
            let addr = self.stack_pop()?;
            self.heap_store(addr, val)?;
            self.pc += 1;
        }
        Instruction::Retrieve => {
            let addr = self.stack_pop()?;
            let val = self.heap_retrieve(addr)?;
            self.data_stack.push(val);
            self.pc += 1;
        }

        // === フロー制御 ===
        Instruction::Label(_) => {
            // ラベルは実行時に何もしない（初期化時に収集済み）
            self.pc += 1;
        }
        Instruction::Call(id) => {
            self.call_stack.push(self.pc + 1);
            self.pc = self.resolve_label(id)?;
        }
        Instruction::Jump(id) => {
            self.pc = self.resolve_label(id)?;
        }
        Instruction::JumpIfZero(id) => {
            let val = self.stack_pop()?;
            if val == 0 {
                self.pc = self.resolve_label(id)?;
            } else {
                self.pc += 1;
            }
        }
        Instruction::JumpIfNegative(id) => {
            let val = self.stack_pop()?;
            if val < 0 {
                self.pc = self.resolve_label(id)?;
            } else {
                self.pc += 1;
            }
        }
        Instruction::Return => {
            self.pc = self.call_stack.pop()
                .ok_or(RuntimeError::CallStackUnderflow)?;
        }
        Instruction::Exit => {
            return ExecuteResult::Exit;
        }

        // === I/O ===
        Instruction::OutputChar => {
            let val = self.stack_pop()?;
            write!(self.stdout, "{}", (val as u8) as char)
                .map_err(|e| RuntimeError::IoError(e.to_string()))?;
            self.pc += 1;
        }
        Instruction::OutputNumber => {
            let val = self.stack_pop()?;
            write!(self.stdout, "{}", val)
                .map_err(|e| RuntimeError::IoError(e.to_string()))?;
            self.pc += 1;
        }
        Instruction::InputChar => {
            let addr = self.stack_pop()?;
            let val = self.read_char()?;
            self.heap.insert(addr, val);
            self.pc += 1;
        }
        Instruction::InputNumber => {
            let addr = self.stack_pop()?;
            let val = self.read_number()?;
            self.heap.insert(addr, val);
            self.pc += 1;
        }
    }

    ExecuteResult::Continue
}
```

### ヘルパーメソッド

```rust
/// スタックからポップ
fn stack_pop(&mut self) -> Result<i64, RuntimeError> {
    self.data_stack.pop().ok_or(RuntimeError::StackUnderflow)
}

/// スタックのトップを参照（ポップしない）
fn stack_top(&self) -> Result<i64, RuntimeError> {
    self.data_stack.last().copied().ok_or(RuntimeError::StackUnderflow)
}

/// 二項演算のヘルパー
fn binary_op<F>(&mut self, op: F) -> Result<(), RuntimeError>
where
    F: FnOnce(i64, i64) -> Result<i64, RuntimeError>,
{
    let b = self.stack_pop()?;
    let a = self.stack_pop()?;
    let result = op(a, b)?;
    self.data_stack.push(result);
    self.pc += 1;
    Ok(())
}

/// ラベルを解決
fn resolve_label(&self, id: &LabelId) -> Result<usize, RuntimeError> {
    let key = id.to_ws_value();
    self.labels.get(&key).copied()
        .ok_or(RuntimeError::UndefinedLabel(key))
}
```

### ヒープアクセスと拡張 API

```rust
/// ヒープへの書き込み（拡張 API フック付き）
fn heap_store(&mut self, addr: i64, val: i64) -> Result<(), RuntimeError> {
    match addr {
        // 拡張 API: 負アドレスへの書き込みを特殊操作として解釈
        -1 => {
            // __trace(val)
            let traced = &mut self.traced;
            if let Some(v) = traced.get_mut(&val) {
                *v += 1;
            } else {
                traced.insert(val, 1);
            }
        }
        -2 => {
            // __assert(val): val == 0 ならエラー
            if val == 0 {
                return Err(RuntimeError::AssertionFailed(val));
            }
        }
        -3 => {
            // __assert_not(val): val != 0 ならエラー
            if val != 0 {
                return Err(RuntimeError::AssertionFailed(val));
            }
        }
        _ => {
            // 通常のヒープ書き込み
            self.heap.insert(addr, val);
        }
    }
    Ok(())
}

/// ヒープからの読み出し
fn heap_retrieve(&self, addr: i64) -> Result<i64, RuntimeError> {
    // 未初期化アドレスは 0 を返す（Whitespace の一般的な挙動）
    Ok(*self.heap.get(&addr).unwrap_or(&0))
}
```

### 中断・再開のデータフロー

```
呼び出し元 (JS / CLI)
    │
    ▼
vm.step(10000)
    │
    ▼
┌─────────────────────────────┐
│ for _ in 0..10000 {         │
│     execute_instruction()   │    ← 1命令ずつ実行
│     total_steps += 1        │
│ }                           │
│ return Suspended            │    ← budget 消費で中断
└─────────────┬───────────────┘
              │
    ▼ (制御が呼び出し元に戻る)
    
(UIの更新、進捗表示等)

    │
    ▼
vm.step(10000)               ← 再開: pc, stack, heap はそのまま保持
    │
    ▼
┌─────────────────────────────┐
│ for _ in 0..10000 {         │    ← 前回の pc 位置から実行再開
│     execute_instruction()   │
│ }                           │
│ return Complete / Suspended │
└─────────────────────────────┘
```

**ポイント**: 全ての実行状態は `WhitespaceVM` struct のフィールドに保持されるため、
関数の戻り = 自然な中断、関数の再呼び出し = 自然な再開 となる。
Continuation や ContinuationFrame のような複雑な仕組みは**一切不要**。

### Whitespace テキストのパース

パーサは Whitespace テキスト（Space/Tab/LF の列）を `Vec<Instruction>` に変換する。

```rust
// src/whitespace/parser.rs

/// パースエラー
#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// 不正な命令バイト列
    InvalidInstruction(usize),
    /// 予期しないファイル終端
    UnexpectedEof,
    /// 数値パースエラー
    InvalidNumber(usize),
}

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
```

パーサの実装は IMP プレフィックスに基づくツリーウォークで行う:

```rust
fn parse_instruction(chars: &[WsChar], pos: usize) -> Result<(Instruction, usize), ParseError> {
    match chars.get(pos) {
        Some(WsChar::Space) => parse_stack_op(chars, pos + 1),
        Some(WsChar::Tab) => match chars.get(pos + 1) {
            Some(WsChar::Space) => parse_arithmetic(chars, pos + 2),
            Some(WsChar::Tab) => parse_heap(chars, pos + 2),
            Some(WsChar::Lf) => parse_io(chars, pos + 2),
            None => Err(ParseError::UnexpectedEof),
        },
        Some(WsChar::Lf) => parse_flow(chars, pos + 1),
        None => Err(ParseError::UnexpectedEof),
    }
}
```

## 使用例

### コンパイル → Whitespace VM 実行のパイプライン

```rust
let scope = syntactic_analyze(&stmts)?;
let program = compiler_ws::compile(&scope)?;
let mut vm = WhitespaceVM::from_instructions(program.into_instructions());

// 一括実行
let result = vm.run(1_000_000);
assert_eq!(result, StepResult::Complete);
assert_eq!(vm.traced, expected_traces);
```

### Whitespace テキストの直接実行

```rust
let source = include_str!("program.ws");
let mut vm = WhitespaceVM::from_source(source)?;
let result = vm.run(1_000_000);
```

### CLI

```bash
# Whitespace ファイルを直接実行
cargo run --bin whitespace20 -- program.ws

# 入力ファイル指定
cargo run --bin whitespace20 -- program.ws -i input.txt

# nospace → Whitespace → 実行 のパイプライン
cargo run --bin nospace20 -- source.ns --mode compile -o program.ws
cargo run --bin whitespace20 -- program.ws
```

## 既存モジュールとの接点

### compiler_ws との関係

```
compiler_ws::Instruction ←── 型を共有 ──→ whitespace::Instruction (re-export)
compiler_ws::WsProgram   ─── into_instructions() ──→ Vec<Instruction>
                                                          │
                                                          ▼
                                                   WhitespaceVM::from_instructions()
```

`WsProgram` に `into_instructions()` メソッドを追加し、内部の `Vec<Instruction>` を取り出せるようにする。

### interpreter (nospace) との関係

機能的に独立。nospace インタプリタは nospace AST を直接実行し、
Whitespace インタプリタは Whitespace バイトコードを実行する。

共通点は以下の動作仕様のみ:
- `__trace` の BTreeMap<i64, i64> への記録方式
- I/O のバッファリング方式 (BufRead / Write trait)
- 中断可能実行の API パターン (step + StepResult)

## 変更対象ファイル一覧

| ファイル | 変更種別 | 内容 |
|---------|---------|------|
| `src/whitespace/mod.rs` | **新規** | モジュール定義、re-export |
| `src/whitespace/parser.rs` | **新規** | Whitespace テキスト → Vec\<Instruction\> |
| `src/whitespace/interpreter.rs` | **新規** | WhitespaceVM、StepResult、実行ロジック |
| `src/bin/whitespace20.rs` | **新規** | Whitespace インタプリタ CLI |
| `src/compiler_ws/program.rs` | **変更** | `into_instructions()` メソッド追加 |
| `src/lib.rs` | **変更** | `mod whitespace` 追加、公開 API 追加 |

## 設計上のトレードオフ

### ヒープの未初期化アクセス

| 選択肢 | 動作 |
|--------|------|
| A: 0 を返す | 多くの Whitespace 実装と互換。安全だがバグを隠す可能性 |
| B: エラーを返す | 厳密だがプログラムが正しく動かない場合がある |

**方針: A (0 を返す)** — Whitespace の一般的な挙動に合わせる。`compiler_ws` が初期化なしのアドレスにアクセスするコードを生成し得るため。

### Copy(n) / Slide(n) の n の範囲チェック

**方針: 範囲外はスタックアンダーフローエラー** — Whitespace 仕様に準拠。

### I/O 命令の即時フラッシュ

**方針: 出力命令ごとにはフラッシュしない** — パフォーマンスのため。`flush()` メソッドで明示的にフラッシュ。CLI では実行完了後にフラッシュ。

## 統合テスト設計

### 方針: wsc との結果比較

統合テストでは、同じ Whitespace コードを **whitespace20 (自前実装)** と **wsc (外部ツール)** の両方で実行し、結果が一致することを検証する。

```
[Whitespace コード] ──→ whitespace20 ──→ stdout_a
                    └─→ wsc          ──→ stdout_b
                    
assert_eq!(stdout_a, stdout_b)
```

### テスト分類

| テスト種別 | 手法 | 対象 |
|-----------|------|------|
| **Unit テスト** | `#[cfg(test)]` 各モジュール内 | 個別命令、パーサ、VM 状態遷移 |
| **統合テスト (自前のみ)** | `compile → WhitespaceVM::run` | nospace → WS → VM 実行の trace/IO 検証 |
| **統合テスト (wsc 比較)** | `compile → whitespace20 / wsc` | stdout が一致するか比較 |

### wsc 比較テストの実装

既存の `tests/common/mod.rs` に `run_whitespace()` (wsc 実行) が実装済み。
これと同様に `run_whitespace20()` (自前 VM 実行) を追加し比較する。

```rust
// tests/common/mod.rs に追加

/// 自前の Whitespace VM でコードを実行
pub fn run_whitespace_vm(ws_code: &str, stdin_input: &str) -> Result<String, String> {
    use nospace20::whitespace::{WhitespaceVM, StepResult};
    use std::io::{BufReader, Cursor};
    
    let mut vm = WhitespaceVM::from_source(ws_code)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    let stdin = Box::new(BufReader::new(Cursor::new(stdin_input.as_bytes().to_vec())));
    let stdout: Box<dyn std::io::Write> = Box::new(Vec::<u8>::new());
    vm = vm.with_io(stdin, stdout);
    
    let result = vm.run(1_000_000);
    vm.flush();
    
    match result {
        StepResult::Complete => { /* OK */ }
        StepResult::Error(e) => return Err(format!("Runtime error: {:?}", e)),
        StepResult::Suspended => return Err("Execution limit exceeded".into()),
    }
    
    // stdout バッファから文字列を取得
    // (実装時に適切な API を提供)
    Ok(vm.get_stdout_string())
}
```

#### テストケース

```rust
// tests/whitespace_vm_test.rs (新規)

/// nospace → Whitespace コンパイル → 自前 VM 実行 → wsc 実行 → 結果比較
fn test_whitespace_vm_vs_wsc(test_name: &str, stdin: &str) {
    let path_base = format!("resources/tests/passes/{}", test_name);
    let ns_cnt = fs::read_to_string(format!("{}.ns", path_base)).unwrap();
    
    // nospace → Whitespace コンパイル
    let t = parse_to_tokens(&ns_cnt).unwrap();
    let s = parse_to_tree(&t).unwrap();
    let a = syntactic_analyze(&s).unwrap();
    let ws_code = compile_to_whitespace(&a).unwrap();
    
    // 自前 VM で実行
    let vm_stdout = run_whitespace_vm(&ws_code, stdin).unwrap();
    
    // wsc で実行（利用可能な場合）
    if wsc_available() {
        let wsc_stdout = run_whitespace(&ws_code, stdin).unwrap();
        assert_eq!(
            vm_stdout, wsc_stdout,
            "whitespace20 and wsc output differ for test '{}'",
            test_name
        );
    }
}
```

### テスト生成 (test-manifest.yaml 拡張)

既存の `targets: [interpreter, whitespace]` の `whitespace` ターゲットを拡張し、
wsc に加えて自前 VM でも実行する。

```yaml
# test-manifest.yaml
tests:
  - name: test_io_puti_basic_001
    type: success_io
    path: io/puti_basic_001
    targets:
      - interpreter       # nospace インタプリタ
      - whitespace         # wsc (外部ツール)
      - whitespace_vm      # 自前 WhitespaceVM (新規)
```

### Slide 命令の Instruction 追加

現在の `compiler_ws::instruction::Instruction` に `Slide` が未定義の場合、
パーサで whitespace テキストの Slide 命令 (`[S][T][LF]<n>`) をパースするために追加が必要。

```rust
// src/compiler_ws/instruction.rs に追加
Slide(WsNumber),    // SP TB LF <n>
```

ただし、`compiler_ws` がコード生成で Slide を使用しない場合、
パーサ側でのみ対応し、Instruction enum 自体は将来必要になった時点で追加する方針でもよい。
