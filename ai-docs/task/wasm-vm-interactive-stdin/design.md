# 詳細設計: WASM WhitespaceVM Interactive Stdin

## 1. 概念

```
JS 側                                   Rust (WASM)
──────                                  ───────────
vm.step(1000)  ──────────────────────>  step() 実行
               <──────────────────────  { status: "waiting_for_input", inputType: "char" }
                                        (PC は InputChar 命令を指したまま)

vm.provide_stdin("A")  ───────────────> InteractiveStdin バッファに "A" を追加

vm.step(1000)  ──────────────────────>  step() 再開
                                        InputChar 命令をリトライ → バッファから 'A' を消費
               <──────────────────────  { status: "suspended" }
```

## 2. 型定義の変更

### 2.1 StepResult (src/whitespace/interpreter.rs)

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum StepResult {
    Suspended,
    Complete,
    Error(RuntimeError),
    /// stdin バッファ不足による一時停止
    WaitingForInput(InputWaitType),
}

/// 入力待ちの種別
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InputWaitType {
    /// InputChar 命令（1文字入力待ち）
    Char,
    /// InputNumber 命令（数値入力待ち＝1行入力待ち）
    Number,
}
```

### 2.2 ExecuteResult (内部用)

```rust
enum ExecuteResult {
    Continue,
    Exit,
    Error(RuntimeError),
    /// 入力待ちで一時停止
    WaitingForInput(InputWaitType),
}
```

## 3. InteractiveStdin

### 3.1 設計方針

現在の WhitespaceVM は `stdin: Box<dyn BufRead>` を保持している。
interactive モードのための新しい stdin 実装を提供する。

```rust
/// 追記可能な stdin バッファ
///
/// BufRead を実装し、バッファが空の場合は WouldBlock エラーを返す。
/// WouldBlock を受け取った VM はそれを WaitingForInput に変換する。
pub struct InteractiveStdin {
    buffer: Rc<RefCell<Vec<u8>>>,
    position: Rc<RefCell<usize>>,
}
```

### 3.2 動作

- `read()`: バッファに未読データがあればそれを返す。なければ `io::ErrorKind::WouldBlock` を返す
- `read_line()`: バッファに `\n` を含む未読データがあればそれを返す。なければ `WouldBlock` を返す
- `append()`: 外部（WASM API）からデータを追加する

### 3.3 BufRead 実装の詳細

```rust
impl std::io::Read for InteractiveStdin {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let buffer = self.buffer.borrow();
        let pos = *self.position.borrow();
        let remaining = &buffer[pos..];

        if remaining.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "no input available",
            ));
        }

        let to_read = remaining.len().min(buf.len());
        buf[..to_read].copy_from_slice(&remaining[..to_read]);
        drop(buffer);
        *self.position.borrow_mut() += to_read;
        Ok(to_read)
    }
}

impl std::io::BufRead for InteractiveStdin {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        // 注: Rc<RefCell> 経由のため fill_buf の直接実装は困難
        // read_line は BufRead のデフォルト実装が read を使うため、
        // fill_buf + consume パターンの代わりに
        // read_line をオーバーライドして実装する
        todo!("fill_buf の実装は後述")
    }

    fn consume(&mut self, amt: usize) {
        *self.position.borrow_mut() += amt;
    }
}
```

### 3.4 代替案: BufRead を使わない方針

`Rc<RefCell<...>>` と `BufRead` トレイトの `fill_buf` のライフタイム要件は相性が悪い。
代替として **VM 内部で直接 InteractiveStdin を扱う** 方針がより現実的。

```rust
pub struct WhitespaceVM {
    // stdin を2種類のどちらかに切り替え可能
    stdin: StdinSource,
    // ...
}

enum StdinSource {
    /// 従来の BufRead ベース（非 interactive / テスト用）
    Buffered(Box<dyn BufRead>),
    /// Interactive モード（追記可能バッファ）
    Interactive(InteractiveBuffer),
}

struct InteractiveBuffer {
    data: Vec<u8>,
    position: usize,
}

impl InteractiveBuffer {
    fn append(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    /// 1バイト読み取り。バッファ不足時は None を返す
    fn read_byte(&mut self) -> Option<u8> {
        if self.position < self.data.len() {
            let b = self.data[self.position];
            self.position += 1;
            Some(b)
        } else {
            None
        }
    }

    /// 1行読み取り。改行を含む行がバッファにない場合は None を返す
    fn read_line(&mut self) -> Option<String> {
        let remaining = &self.data[self.position..];
        if let Some(newline_pos) = remaining.iter().position(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(&remaining[..=newline_pos]).to_string();
            self.position += newline_pos + 1;
            Some(line)
        } else {
            None
        }
    }
}
```

**推奨: 代替案を採用**。`StdinSource` enum で従来の `Box<dyn BufRead>` と `InteractiveBuffer` を切り替える。

## 4. WhitespaceVM の変更

### 4.1 フィールド変更

```rust
pub struct WhitespaceVM {
    // stdin: Box<dyn BufRead>,   ← 削除
    stdin: StdinSource,            // ← 新規
    // ... 他は変更なし
}
```

### 4.2 InputChar / InputNumber の変更

```rust
Instruction::InputChar => {
    let addr = match self.stack_pop() {
        Ok(v) => v,
        Err(e) => return ExecuteResult::Error(e),
    };
    match self.read_char() {
        Ok(v) => {
            self.heap.insert(addr, v);
            self.pc += 1;
        }
        Err(ReadResult::WouldBlock) => {
            // スタックにアドレスを戻して一時停止
            self.data_stack.push(addr);
            return ExecuteResult::WaitingForInput(InputWaitType::Char);
        }
        Err(ReadResult::IoError(e)) => {
            return ExecuteResult::Error(RuntimeError::IoError(e));
        }
    }
}
```

### 4.3 read_char / read_number の変更

```rust
enum ReadResult {
    WouldBlock,
    IoError(String),
}

fn read_char(&mut self) -> Result<i64, ReadResult> {
    match &mut self.stdin {
        StdinSource::Buffered(reader) => {
            let mut buf = [0u8; 1];
            match reader.read(&mut buf) {
                Ok(1) => Ok(buf[0] as i64),
                Ok(_) => Ok(0), // EOF
                Err(e) => Err(ReadResult::IoError(e.to_string())),
            }
        }
        StdinSource::Interactive(buffer) => {
            match buffer.read_byte() {
                Some(b) => Ok(b as i64),
                None => Err(ReadResult::WouldBlock),
            }
        }
    }
}

fn read_number(&mut self) -> Result<i64, ReadResult> {
    match &mut self.stdin {
        StdinSource::Buffered(reader) => {
            let mut line = String::new();
            reader.read_line(&mut line)
                .map_err(|e| ReadResult::IoError(e.to_string()))?;
            line.trim().parse::<i64>()
                .map_err(|e| ReadResult::IoError(e.to_string()))
        }
        StdinSource::Interactive(buffer) => {
            match buffer.read_line() {
                Some(line) => {
                    line.trim().parse::<i64>()
                        .map_err(|e| ReadResult::IoError(e.to_string()))
                }
                None => Err(ReadResult::WouldBlock),
            }
        }
    }
}
```

### 4.4 step() の変更

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
            ExecuteResult::WaitingForInput(input_type) => {
                return StepResult::WaitingForInput(input_type);
            }
        }
    }

    StepResult::Suspended
}
```

### 4.5 ビルダーメソッド追加

```rust
/// Interactive stdin モードで構築（WASM 用）
///
/// stdin バッファが空の場合に WaitingForInput を返すようになる。
/// provide_stdin() で後からデータを追加可能。
pub fn with_interactive_stdin(mut self) -> Self {
    self.stdin = StdinSource::Interactive(InteractiveBuffer::new());
    self
}

/// Interactive stdin にデータを追加
///
/// WaitingForInput 状態の後に呼び出し、次の step() で入力をリトライする。
pub fn provide_stdin(&mut self, data: &str) {
    if let StdinSource::Interactive(buffer) = &mut self.stdin {
        buffer.append(data.as_bytes());
    }
}
```

## 5. WASM API の変更 (src/wasm_api.rs)

### 5.1 TypeScript 型定義の更新

```typescript
interface VmStepResult {
    status: "suspended" | "complete" | "error" | "waiting_for_input";
    error?: string;
    inputType?: "char" | "number";
}
```

### 5.2 WasmWhitespaceVM の拡張

```rust
#[wasm_bindgen]
impl WasmWhitespaceVM {
    /// Interactive モード対応のコンストラクタ
    ///
    /// interactive=true の場合、stdin が不足すると WaitingForInput で一時停止する。
    /// interactive=false の場合、従来通り stdin を事前提供する。
    #[wasm_bindgen(constructor)]
    pub fn new(
        nospace_source: &str,
        stdin: &str,
        interactive: Option<bool>,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        // ... コンパイル処理は同じ ...

        if interactive.unwrap_or(false) {
            Self::from_whitespace_interactive(&ws_source, stdin)
        } else {
            Self::from_whitespace(&ws_source, stdin)
        }
    }

    /// Interactive モードで Whitespace ソースから VM を構築
    #[wasm_bindgen(js_name = "fromWhitespaceInteractive")]
    pub fn from_whitespace_interactive(
        ws_source: &str,
        initial_stdin: &str,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        let vm = WhitespaceVM::from_source(ws_source)
            .map_err(/* ... */)?
            .with_debug_ext(false)
            .with_interactive_stdin();

        // 初期データがあれば投入
        if !initial_stdin.is_empty() {
            vm.provide_stdin(initial_stdin);
        }

        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_clone = Rc::clone(&stdout_buf);
        let vm_with_io = vm.with_stdout(Box::new(SharedWriter(stdout_clone)));

        Ok(WasmWhitespaceVM {
            vm: vm_with_io,
            stdout_buffer: stdout_buf,
        })
    }

    /// stdin にデータを追加する（interactive モード用）
    ///
    /// WaitingForInput 状態の際に呼び出し、次の step() で入力を再試行する。
    #[wasm_bindgen(js_name = "provideStdin")]
    pub fn provide_stdin(&mut self, data: &str) {
        self.vm.provide_stdin(data);
    }
}
```

### 5.3 step() の VmStepResult 更新

```rust
pub fn step(&mut self, budget: u32) -> JsVmStepResult {
    let result = self.vm.step(budget as usize);

    let vm_result = match result {
        StepResult::Suspended => VmStepResult {
            status: "suspended".to_string(),
            error: None,
            input_type: None,
        },
        StepResult::Complete => VmStepResult {
            status: "complete".to_string(),
            error: None,
            input_type: None,
        },
        StepResult::Error(e) => VmStepResult {
            status: "error".to_string(),
            error: Some(format!("{:?}", e)),
            input_type: None,
        },
        StepResult::WaitingForInput(input_type) => VmStepResult {
            status: "waiting_for_input".to_string(),
            error: None,
            input_type: Some(match input_type {
                InputWaitType::Char => "char".to_string(),
                InputWaitType::Number => "number".to_string(),
            }),
        },
    };

    let js: JsValue = serde_wasm_bindgen::to_value(&vm_result).unwrap();
    js.into()
}
```

## 6. with_io との整合性

現在の `with_io()` は stdin と stdout を同時に設定する:

```rust
pub fn with_io(mut self, stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self
```

interactive mode では stdin を `StdinSource::Interactive` にするため、stdout のみ別途設定するメソッドが必要:

```rust
/// stdout のみを設定する（interactive stdin モードと併用）
pub fn with_stdout(mut self, stdout: Box<dyn Write>) -> Self {
    self.stdout = stdout;
    self
}
```

`with_io()` は非 interactive モードでは従来通り使用可能。内部で `StdinSource::Buffered` に変換:

```rust
pub fn with_io(mut self, stdin: Box<dyn BufRead>, stdout: Box<dyn Write>) -> Self {
    self.stdin = StdinSource::Buffered(stdin);
    self.stdout = stdout;
    self
}
```

## 7. 既存動作への影響

### 非 interactive モード（デフォルト）

- `StdinSource::Buffered` を使用
- `read_char` / `read_number` は従来通り `BufRead` から読む
- `WaitingForInput` は発生しない
- **動作変更なし**

### テスト

- 既存のユニットテスト・統合テストは `StdinSource::Buffered` パスを使うため影響なし
- `StepResult` enum にバリアントが増えるため、match 文の網羅性で **コンパイルエラーが出る場所があれば修正が必要**

### CLI

- CLI は `with_io()` を使うため `StdinSource::Buffered` パスとなり、影響なし

## 8. 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/whitespace/interpreter.rs` | `StepResult`, `ExecuteResult`, `StdinSource`, `InteractiveBuffer`, `read_char`/`read_number` 分岐, ビルダーメソッド |
| `src/whitespace/mod.rs` | `InputWaitType` の re-export |
| `src/wasm_api.rs` | TypeScript 型定義更新, `provide_stdin`, `from_whitespace_interactive`, `VmStepResult` フィールド追加 |
| `tests/` | `StepResult` パターンマッチの更新（必要に応じて） |

## 9. 検討事項

### 9.1 コンストラクタの互換性

`WasmWhitespaceVM::new()` のシグネチャに `interactive` 引数を追加すると、既存の JS 呼び出しが壊れる可能性がある。

**対策**: `Option<bool>` にして省略可能にするか、別コンストラクタ（ `newInteractive()` ）を追加する。`Option<bool>` が推奨。`wasm_bindgen` では `Option<bool>` は JS 側で省略可能な引数になる。

### 9.2 InputNumber の改行待ち

`InputNumber` は `read_line` を使うため、改行（`\n`）がバッファにない限り `WaitingForInput` を返し続ける。JS 側で `provide_stdin("42\n")` のように改行付きでデータを投入する必要がある。これは TypeScript 型定義のドキュメントコメントで明示する。

### 9.3 消費済みバッファの解放

`InteractiveBuffer` の `data: Vec<u8>` は追記され続けるため、長時間実行ではメモリ使用量が増加する。定期的に消費済みデータを解放するメソッドを検討:

```rust
impl InteractiveBuffer {
    /// 消費済みデータを解放してメモリを回収
    fn compact(&mut self) {
        if self.position > 0 {
            self.data.drain(..self.position);
            self.position = 0;
        }
    }
}
```

`provide_stdin()` 呼び出し時に自動で compact するのが妥当。
