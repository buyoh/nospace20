# Phase A: Whitespace コンパイル + ステップ実行 API

## 概要

nospace ソースコードを Whitespace にコンパイルし、Whitespace VM でステップ実行する機能を WASM API として公開する。

既存の `compiler_ws`（nospace → Whitespace コンパイラ）と `whitespace::interpreter`（中断可能 Whitespace VM）を組み合わせ、Web 環境で以下のワークフローを実現する：

```
[nospace ソース]
    ↓ compile_to_ws()
[Whitespace コード]
    ↓ WasmWhitespaceVM::new()
[VM インスタンス]
    ↓ vm.step(budget) を繰り返し呼び出し
[実行完了 / 中断中 / エラー]
```

## 前提条件・依存タスク

| タスク | 状態 | 必要度 | 説明 |
|--------|------|--------|------|
| `compiler_ws` | ✅ 完了 | 必須 | nospace → Whitespace コンパイル |
| `whitespace-interpreter` Phase 1 | ✅ 完了 | 必須 | 基本実行エンジン（step(budget) 対応） |
| `whitespace-interpreter` Phase 2 | ✅ 完了 | 必須 | CLI + 拡張 API + I/O |
| `wasm-build` Phase 1（ビルド基盤） | 未着手 | 必須 | Cargo.toml + wasm-bindgen 設定 |
| `whitespace-interpreter` Phase 3（統合テスト） | 未着手 | 推奨 | wsc 比較テスト |

## 新規 WASM API

### WasmWhitespaceVM（ステートフル VM ラッパー）

wasm-bindgen のオペーク型サポートを利用し、`WhitespaceVM` を JS 側にステートフルなオブジェクトとして公開する。

```rust
// src/wasm_api.rs に追加

use crate::whitespace::{WhitespaceVM, StepResult, RuntimeError};

/// Whitespace VM の WASM ラッパー
///
/// JS 側ではオペーク型（内部状態は JS から直接アクセスできない）として扱われ、
/// メソッド呼び出しで状態を操作する。
#[wasm_bindgen]
pub struct WasmWhitespaceVM {
    vm: WhitespaceVM,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}
```

### コンストラクタ

```rust
#[wasm_bindgen]
impl WasmWhitespaceVM {
    /// nospace ソースをコンパイルし、Whitespace VM を構築する
    ///
    /// 内部で以下を実行:
    /// 1. nospace ソース → パース → 意味解析
    /// 2. compiler_ws で Whitespace 命令列に変換
    /// 3. WhitespaceVM を命令列から初期化
    #[wasm_bindgen(constructor)]
    pub fn new(nospace_source: &str, stdin: &str) -> Result<WasmWhitespaceVM, JsValue>;

    /// Whitespace ソースコードから直接 VM を構築する
    #[wasm_bindgen(js_name = "fromWhitespace")]
    pub fn from_whitespace(ws_source: &str, stdin: &str) -> Result<WasmWhitespaceVM, JsValue>;
}
```

### 実行制御メソッド

```rust
#[wasm_bindgen]
impl WasmWhitespaceVM {
    /// 指定ステップ数だけ実行する
    ///
    /// 戻り値: { status: "suspended" | "complete" | "error", error?: string }
    pub fn step(&mut self, budget: u32) -> JsValue;

    /// 現在のプログラムカウンタ（命令インデックス）
    pub fn pc(&self) -> usize;

    /// 総実行命令数
    pub fn total_steps(&self) -> usize;

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool;
}
```

### 状態参照メソッド（デバッガ UI 向け）

```rust
#[wasm_bindgen]
impl WasmWhitespaceVM {
    /// データスタックの現在の内容
    ///
    /// 戻り値: number[] (i64 → JS number に変換。53bit 超は精度が落ちる)
    pub fn get_stack(&self) -> JsValue;

    /// ヒープの現在の内容
    ///
    /// 戻り値: { [address: string]: number } (キーは文字列化した i64)
    pub fn get_heap(&self) -> JsValue;

    /// コールスタックの深さ
    pub fn call_stack_depth(&self) -> usize;

    /// 標準出力バッファの内容を取得しクリアする
    ///
    /// 呼び出し側は定期的にこれを呼んで出力を回収する。
    pub fn flush_stdout(&mut self) -> String;

    /// トレース情報を取得
    ///
    /// 戻り値: { [key: string]: number }
    pub fn get_traced(&self) -> JsValue;

    /// 現在の命令のニーモニック表現を取得（デバッグ用）
    pub fn current_instruction(&self) -> Option<String>;

    /// 命令列全体のニーモニック表現を取得
    pub fn disassemble(&self) -> JsValue;
}
```

### コンパイルのみ（VM を作らずにコンパイル結果を得る）

既存の `compile()` API を活用する。mnemonic ターゲットで人間可読な出力も得られる。

```rust
#[wasm_bindgen]
pub fn compile_to_whitespace(source: &str) -> JsValue;

#[wasm_bindgen]
pub fn compile_to_mnemonic(source: &str) -> JsValue;
```

戻り値は `api-design.md` の `CompileResult` / `ErrorResult` と同一形式。

## JS 側の利用例

### 基本的なステップ実行

```javascript
import { WasmWhitespaceVM } from './pkg/nospace20.js';

const source = `
func: main() {
  __puti(42);
}
`;

const vm = new WasmWhitespaceVM(source, "");

function runLoop() {
  const result = vm.step(1000);
  
  // 出力を回収
  const output = vm.flush_stdout();
  if (output) {
    document.getElementById('output').textContent += output;
  }
  
  if (result.status === 'suspended') {
    // UIフリーズ防止: 次フレームで続行
    requestAnimationFrame(runLoop);
  } else if (result.status === 'complete') {
    console.log('Execution complete');
  } else {
    console.error('Error:', result.error);
  }
}

runLoop();
```

### デバッガ UI との統合

```javascript
const vm = new WasmWhitespaceVM(source, stdin);

// 1命令ずつステップ実行
function stepOne() {
  const result = vm.step(1);
  
  // UI 更新
  updateStackView(vm.get_stack());
  updateHeapView(vm.get_heap());
  updatePcIndicator(vm.pc());
  updateStepCounter(vm.total_steps());
  updateInstructionHighlight(vm.current_instruction());
  
  const output = vm.flush_stdout();
  if (output) appendOutput(output);
  
  return result;
}

// ステップ実行ボタン
document.getElementById('step-btn').onclick = stepOne;

// 実行ボタン（完了まで）
document.getElementById('run-btn').onclick = () => {
  function chunk() {
    const result = vm.step(10000);
    updateUI(vm);
    if (result.status === 'suspended') {
      requestAnimationFrame(chunk);
    }
  }
  chunk();
};
```

## 実装上の考慮事項

### WhitespaceVM の I/O アダプタ

`WhitespaceVM` は `Box<dyn BufRead>` / `Box<dyn Write>` で I/O を抽象化している。
WASM ラッパーでは以下のように構築する：

```rust
impl WasmWhitespaceVM {
    fn build(instructions: Vec<Instruction>, stdin: &str) -> Self {
        use std::cell::RefCell;
        use std::io::Cursor;
        use std::rc::Rc;

        let stdin_buf = Box::new(std::io::BufReader::new(
            Cursor::new(stdin.as_bytes().to_vec())
        ));

        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_clone = Rc::clone(&stdout_buf);

        // SharedWriter pattern（lib.rs の interpret_func_with_io と同じ）
        struct SharedWriter(Rc<RefCell<Vec<u8>>>);
        impl std::io::Write for SharedWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.borrow_mut().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        let vm = WhitespaceVM::from_instructions(instructions)
            .with_io(stdin_buf, Box::new(SharedWriter(stdout_clone)));

        WasmWhitespaceVM { vm, stdout_buffer: stdout_buf }
    }
}
```

### flush_stdout の実装

```rust
pub fn flush_stdout(&mut self) -> String {
    let mut buf = self.stdout_buffer.borrow_mut();
    let text = String::from_utf8_lossy(&buf).to_string();
    buf.clear();
    text
}
```

### i64 ↔ JS Number の変換

Whitespace VM のスタック値は `i64` だが、JS の `Number` は ±2^53 の整数精度しかない。

**Phase A の方針**: `Number` を使用する（多くの nospace プログラムは 53bit 範囲内）。
将来的に `BigInt` 対応が必要な場合は `get_stack_bigint()` 等を追加する。

### エラーハンドリング

`WasmWhitespaceVM::new()` のコンストラクタで発生しうるエラー:

| エラー源 | 内容 |
|---------|------|
| `parse_to_tokens` | 字句解析エラー |
| `parse_to_tree` | 構文解析エラー |
| `syntactic_analyze` | 意味解析エラー |
| `compiler_ws` | コンパイルエラー（未対応機能等） |
| `WhitespaceVM::from_source` | Whitespace パースエラー（from_whitespace 時） |

全て `JsValue` に変換して JS 側に返す。フォーマットは `api-design.md` の `ErrorResult` に準拠。

## WhitespaceVM への追加が必要な機能

現在の `WhitespaceVM` に対し、Phase A を実現するために以下の追加が必要：

| 機能 | 状態 | 説明 |
|------|------|------|
| `step(budget)` | ✅ 実装済 | 中断可能な実行ループ |
| `data_stack()` | ✅ 実装済 | スタック参照 |
| `heap()` | ✅ 実装済 | ヒープ参照 |
| `total_steps()` | ✅ 実装済 | 実行ステップ数 |
| `pc()` | 要追加 | 現在のプログラムカウンタ取得メソッド |
| `call_stack_depth()` | 要追加 | コールスタック深さ取得メソッド |
| `current_instruction()` | 要追加 | 現在の命令のニーモニック |
| `disassemble()` | 要追加 | 命令列の文字列化 |
| `from_instructions(...).with_io(...)` | ✅ 実装済 | 命令列 + I/O 指定の構築 |

追加が必要な項目はいずれも VM の内部状態を参照するだけのシンプルなメソッドであり、
既存ロジックへの影響は最小限。

## フェーズ計画

### Step A-1: ビルド基盤（wasm-build Phase 1 と共通）

- [ ] `Cargo.toml` に `wasm-bindgen` 等の依存追加（feature flag `wasm`）
- [ ] `wasm32-unknown-unknown` でライブラリがコンパイルできることを確認
- [ ] `src/wasm_api.rs` スケルトン作成

### Step A-2: WhitespaceVM の軽微な拡張

- [ ] `pc()` メソッド追加
- [ ] `call_stack_depth()` メソッド追加
- [ ] `current_instruction()` メソッド追加（無くても可）
- [ ] `disassemble()` メソッド追加（無くても可）

### Step A-3: WASM API — コンパイル関数

- [ ] `compile_to_whitespace()` WASM API 実装
- [ ] `compile_to_mnemonic()` WASM API 実装

### Step A-4: WASM API — WasmWhitespaceVM

- [ ] `WasmWhitespaceVM::new()` 実装（nospace → WS → VM）
- [ ] `WasmWhitespaceVM::from_whitespace()` 実装
- [ ] `step()`, `pc()`, `is_complete()`, `total_steps()` 実装
- [ ] `get_stack()`, `get_heap()`, `call_stack_depth()` 実装
- [ ] `flush_stdout()`, `get_traced()` 実装

### Step A-5: テスト・検証

- [ ] Node.js でのスモークテスト（compile → VM 生成 → step 実行）
- [ ] 既存テストケースの一部を WASM 経由で実行・結果照合
- [ ] ブラウザでの動作確認（wasm-pack build --target web）
