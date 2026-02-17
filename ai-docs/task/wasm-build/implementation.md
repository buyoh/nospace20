# 実装手順

## Phase 0: ビルド基盤

### Step 0-1: Cargo.toml 変更

```toml
# 追加
[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = ["cli"]
cli = ["dep:clap", "dep:unicode-width"]
wasm = ["dep:wasm-bindgen", "dep:serde-wasm-bindgen"]

# [dependencies] を修正（clap, unicode-width を optional 化）
clap = { version = "4.0", features = ["derive"], optional = true }
unicode-width = { version = "0.1.8", optional = true }

# [dependencies] に追加
wasm-bindgen = { version = "0.2", optional = true }
serde-wasm-bindgen = { version = "0.6", optional = true }
```

**feature フラグの設計意図:**

| feature | 用途 | 有効化される依存 |
|---------|------|------------------|
| `cli` (default) | CLI バイナリビルド | `clap`, `unicode-width` |
| `wasm` | WASM ライブラリビルド | `wasm-bindgen`, `serde-wasm-bindgen` |

- 通常のビルド (`cargo build`) → `default = ["cli"]` により従来通り動作
- WASM ビルド → `--no-default-features --features wasm` で CLI 向け依存を除外
- `cli` と `wasm` は排他ではないが、同時有効化は想定しない

**確認**: `cargo build` / `cargo test` が従来通り成功すること。

### Step 0-2: .gitignore 更新

```
/pkg/
```

### Step 0-3: wasm32 コンパイル確認

```bash
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --lib --no-default-features --features wasm
```

この時点では `wasm_api.rs` はまだ空でよい。
ライブラリコード全体が wasm32 でコンパイルできることを確認する。

`--no-default-features` により `cli` feature が無効化され、`clap` / `unicode-width` はコンパイル対象外となる。

**潜在的な問題:**
- `std::io::stdin()` / `stdout()` → wasm32-unknown-unknown では no-op 実装が提供されるため、コンパイルは通る
- `build.rs` → ホスト側で実行されるため影響なし
- `clap` / `unicode-width` → `cli` feature 無効時は依存から除外される

### Step 0-4: bin ターゲットの cfg ガード

`clap` と `unicode-width` を optional 化したため、`src/bin/*.rs` では `cli` feature が有効なことを前提とする。

**対応方法:**

Cargo.toml の `[[bin]]` セクションで `required-features` を指定する。これにより `cli` feature が無いときに bin ターゲットはビルド対象から除外される。

```toml
[[bin]]
name = "nospace20"
path = "src/bin/nospace20.rs"
required-features = ["cli"]

[[bin]]
name = "whitespace20"
path = "src/bin/whitespace20.rs"
required-features = ["cli"]
```

**確認:**
- `cargo build` → `default = ["cli"]` により bin がビルドされる
- `cargo build --lib --no-default-features --features wasm` → bin はビルド対象外
- `cargo test` → 従来通り成功

---

## Phase 1: 基本 WASM API（run / compile）

### Step 1-1: wasm_api モジュール作成

**ファイル**: `src/wasm_api.rs`

```rust
//! WebAssembly 公開 API
//!
//! CLI と同等の機能を JavaScript から呼び出し可能にする。
//! `wasm` feature が有効な場合のみコンパイルされる。

use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::{
    compile_to_whitespace, compile_to_whitespace_debug,
    interpret_func_with_io, parse_to_tokens, parse_to_tree,
    syntactic_analyze, CodeParseError, CompileTarget, LanguageStd,
    TextCode,
};
```

### Step 1-2: lib.rs にモジュール登録

```rust
// src/lib.rs に追加
#[cfg(feature = "wasm")]
mod wasm_api;
```

### Step 1-3: 結果型の定義

`src/wasm_api.rs` 内に Serialize 可能な結果型を定義する。

```rust
#[derive(Serialize)]
struct RunResultOk {
    success: bool,           // always true
    #[serde(rename = "returnValue")]
    return_value: Option<i64>,
    stdout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct ResultErr {
    success: bool,           // always false
    errors: Vec<WasmError>,
}

#[derive(Serialize)]
struct CompileResultOk {
    success: bool,           // always true
    output: String,
}

#[derive(Serialize)]
struct WasmError {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}
```

### Step 1-4: エラー変換ヘルパー

```rust
fn convert_errors(errors: &[CodeParseError], text: &TextCode) -> JsValue {
    let wasm_errors: Vec<WasmError> = errors.iter().map(|e| {
        let (line, column) = if let Some(p) = e.code_pointer {
            let (l, c) = text.char_index_to_line(p);
            (Some(l), Some(c))
        } else {
            (None, None)
        };
        WasmError {
            message: e.message.clone(),
            line,
            column,
        }
    }).collect();

    let result = ResultErr {
        success: false,
        errors: wasm_errors,
    };
    serde_wasm_bindgen::to_value(&result).unwrap()
}
```

### Step 1-5: `run` 関数実装

```rust
/// nospace ソースコードを解析・実行する。
/// CLI の `--mode=run` に相当。
#[wasm_bindgen]
pub fn run(source: &str, stdin: &str, debug: bool) -> JsValue {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    // 字句解析
    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text),
    };

    // 構文解析
    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text),
    };

    // 意味解析
    let scope = match syntactic_analyze(&statements) {
        Ok(a) => a,
        Err(errors) => return convert_errors(&errors, &text),
    };

    // 実行
    let (traced, stdout_str) = interpret_func_with_io(&scope, "main", stdin);

    // trace を String キーに変換 (JSON 互換)
    let trace = if debug {
        Some(traced.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    } else {
        None
    };

    // interpret_func_with_io は戻り値を返さないため None とする
    // TODO: interpret_with_io で戻り値も取得できるようにする
    let result = RunResultOk {
        success: true,
        return_value: None,
        stdout: stdout_str,
        trace,
    };
    serde_wasm_bindgen::to_value(&result).unwrap()
}
```

**注意**: 現在の `interpret_func_with_io` は `main` 関数の戻り値を返さない。
戻り値が必要な場合は `lib.rs` に新しいヘルパー関数を追加する必要がある。

### Step 1-6: `compile` 関数実装

```rust
/// nospace ソースコードをコンパイルする。
/// CLI の `--mode=compile` に相当。
#[wasm_bindgen]
pub fn compile(source: &str, target: &str, lang_std: &str) -> JsValue {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    // パラメータ変換
    let compile_target = match target {
        "ws" => CompileTarget::Ws,
        "mnemonic" => CompileTarget::Mnemonic,
        _ => {
            let result = ResultErr {
                success: false,
                errors: vec![WasmError {
                    message: format!("unsupported target: '{}' (use 'ws' or 'mnemonic')", target),
                    line: None,
                    column: None,
                }],
            };
            return serde_wasm_bindgen::to_value(&result).unwrap();
        }
    };

    let language_std = match lang_std {
        "ws" => LanguageStd::Ws,
        "standard" => LanguageStd::Standard,
        _ => {
            let result = ResultErr {
                success: false,
                errors: vec![WasmError {
                    message: format!("unsupported std: '{}' (use 'standard' or 'ws')", lang_std),
                    line: None,
                    column: None,
                }],
            };
            return serde_wasm_bindgen::to_value(&result).unwrap();
        }
    };

    // バリデーション
    if matches!(compile_target, CompileTarget::Ws | CompileTarget::Mnemonic)
        && language_std != LanguageStd::Ws
    {
        let result = ResultErr {
            success: false,
            errors: vec![WasmError {
                message: format!(
                    "target='{}' requires std='ws'",
                    target
                ),
                line: None,
                column: None,
            }],
        };
        return serde_wasm_bindgen::to_value(&result).unwrap();
    }

    // 解析
    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text),
    };
    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text),
    };
    let scope = match syntactic_analyze(&statements) {
        Ok(a) => a,
        Err(errors) => return convert_errors(&errors, &text),
    };

    // コンパイル
    let compiled = match compile_target {
        CompileTarget::Ws => compile_to_whitespace(&scope),
        CompileTarget::Mnemonic => compile_to_whitespace_debug(&scope),
        _ => unreachable!(),
    };

    match compiled {
        Ok(output) => {
            let result = CompileResultOk {
                success: true,
                output,
            };
            serde_wasm_bindgen::to_value(&result).unwrap()
        }
        Err(err) => {
            let result = ResultErr {
                success: false,
                errors: vec![WasmError {
                    message: err,
                    line: None,
                    column: None,
                }],
            };
            serde_wasm_bindgen::to_value(&result).unwrap()
        }
    }
}
```

### Step 1-7: `parse` 関数実装（オプション）

```rust
/// nospace ソースコードの構文チェックのみ行う。
#[wasm_bindgen]
pub fn parse(source: &str) -> JsValue {
    let text = TextCode::new(source);
    let source_string = source.to_string();

    let tokens = match parse_to_tokens(&source_string) {
        Ok(t) => t,
        Err(errors) => return convert_errors(&errors, &text),
    };

    let statements = match parse_to_tree(&tokens) {
        Ok(s) => s,
        Err(errors) => return convert_errors(&errors, &text),
    };

    match syntactic_analyze(&statements) {
        Ok(_) => {
            let result = serde_json::json!({ "success": true });
            serde_wasm_bindgen::to_value(&result).unwrap()
        }
        Err(errors) => convert_errors(&errors, &text),
    }
}
```

### Step 1-8: wasm-pack build & スモークテスト

```bash
wasm-pack build --target nodejs --no-default-features --features wasm
```

成功すれば `pkg/` に出力される。

`tmp/test_wasm.mjs`（.gitignore 済みの tmp/ に配置）:

```javascript
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const nospace = require('../pkg/nospace20');

// テスト 1: 基本的な実行
const r1 = nospace.run('func:main(){__puti(42);}', '', false);
console.assert(r1.success === true);
console.assert(r1.stdout === '42');
console.log('Test 1 passed:', r1);

// テスト 2: stdin 付き実行
const r2 = nospace.run('func:main(){let:x;x=__geti();__puti(x);}', '123', false);
console.assert(r2.success === true);
console.assert(r2.stdout === '123');
console.log('Test 2 passed:', r2);

// テスト 3: パースエラー
const r3 = nospace.run('func:main(){', '', false);
console.assert(r3.success === false);
console.assert(r3.errors.length > 0);
console.log('Test 3 passed:', r3);

// テスト 4: コンパイル
const r4 = nospace.compile('func:main(){__puti(42);}', 'mnemonic', 'ws');
console.assert(r4.success === true);
console.assert(r4.output.length > 0);
console.log('Test 4 passed:', r4);

console.log('All smoke tests passed!');
```

---

## Phase A: Whitespace コンパイル + ステップ実行 API

nospace → Whitespace コンパイル + Whitespace VM ステップ実行の WASM API。
既存の `compiler_ws` + `whitespace::interpreter` を活用する。

### 前提条件・依存タスク

| タスク | 状態 | 必要度 | 説明 |
|--------|------|--------|------|
| `compiler_ws` | ✅ 完了 | 必須 | nospace → Whitespace コンパイル |
| `whitespace-interpreter` Phase 1 | ✅ 完了 | 必須 | 基本実行エンジン（step(budget) 対応） |
| `whitespace-interpreter` Phase 2 | ✅ 完了 | 必須 | CLI + 拡張 API + I/O |
| Phase 0（ビルド基盤） | 未着手 | 必須 | Cargo.toml + wasm-bindgen 設定 |
| `whitespace-interpreter` Phase 3（統合テスト） | 未着手 | 推奨 | wsc 比較テスト |

### ワークフロー

```
[nospace ソース]
    ↓ compile_to_ws()
[Whitespace コード]
    ↓ WasmWhitespaceVM::new()
[VM インスタンス]
    ↓ vm.step(budget) を繰り返し呼び出し
[実行完了 / 中断中 / エラー]
```

### Step A-1: WhitespaceVM の軽微な拡張

現在の `WhitespaceVM` に対し、以下のメソッドを追加する。
いずれも VM の内部状態を参照するだけのシンプルなメソッドであり、既存ロジックへの影響は最小限。

| 機能 | 状態 | 説明 |
|------|------|------|
| `step(budget)` | ✅ 実装済 | 中断可能な実行ループ |
| `data_stack()` | ✅ 実装済 | スタック参照 |
| `heap()` | ✅ 実装済 | ヒープ参照 |
| `total_steps()` | ✅ 実装済 | 実行ステップ数 |
| `pc()` | 要追加 | 現在のプログラムカウンタ取得メソッド |
| `call_stack_depth()` | 要追加 | コールスタック深さ取得メソッド |
| `current_instruction()` | 要追加 | 現在の命令のニーモニック（無くても可） |
| `disassemble()` | 要追加 | 命令列の文字列化（無くても可） |
| `from_instructions(...).with_io(...)` | ✅ 実装済 | 命令列 + I/O 指定の構築 |

### Step A-2: WasmWhitespaceVM 型定義

```rust
// src/wasm_api.rs に追加

use std::cell::RefCell;
use std::rc::Rc;
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

### Step A-3: コンストラクタ

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

### Step A-4: 実行制御メソッド

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

### Step A-5: 状態参照メソッド（デバッガ UI 向け）

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

### Step A-6: コンパイルのみ API

既存の `compile()` API を活用する。mnemonic ターゲットで人間可読な出力も得られる。

```rust
#[wasm_bindgen]
pub fn compile_to_whitespace(source: &str) -> JsValue;

#[wasm_bindgen]
pub fn compile_to_mnemonic(source: &str) -> JsValue;
```

戻り値は Phase 1 の `CompileResult` / `ErrorResult` と同一形式。

### I/O アダプタの実装詳細

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

### flush_stdout

```rust
pub fn flush_stdout(&mut self) -> String {
    let mut buf = self.stdout_buffer.borrow_mut();
    let text = String::from_utf8_lossy(&buf).to_string();
    buf.clear();
    text
}
```

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

### i64 ↔ JS Number の変換

Whitespace VM のスタック値は `i64` だが、JS の `Number` は ±2^53 の整数精度しかない。

**Phase A の方針**: `Number` を使用する（多くの nospace プログラムは 53bit 範囲内）。
将来的に `BigInt` 対応が必要な場合は `get_stack_bigint()` 等を追加する。

### JS 側の利用例: 基本的なステップ実行

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
    requestAnimationFrame(runLoop);
  } else if (result.status === 'complete') {
    console.log('Execution complete');
  } else {
    console.error('Error:', result.error);
  }
}

runLoop();
```

### JS 側の利用例: デバッガ UI

```javascript
const vm = new WasmWhitespaceVM(source, stdin);

// 1命令ずつステップ実行
function stepOne() {
  const result = vm.step(1);
  
  updateStackView(vm.get_stack());
  updateHeapView(vm.get_heap());
  updatePcIndicator(vm.pc());
  updateStepCounter(vm.total_steps());
  updateInstructionHighlight(vm.current_instruction());
  
  const output = vm.flush_stdout();
  if (output) appendOutput(output);
  
  return result;
}

document.getElementById('step-btn').onclick = stepOne;

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

### Step A-7: テスト・検証

- [x] wasm-pack build 成功
- [x] 既存の Rust テストがパス（cargo test --lib: 119 passed）
- [x] テストスクリプト作成（tmp/test_wasm_phase_a.mjs）
- [ ] Node.js でのスモークテスト（WSL 環境の node の問題により未実施）
- [ ] ブラウザでの動作確認（未実施）

**実装済み（2026-02-10）**

---

## Phase B: nospace ステップ実行インタプリタ API

nospace を直接ステップ実行する中断可能インタプリタの WASM API。
`suspendable-interpreter` タスクの完了が前提条件。

### Phase A との比較

| 項目 | Phase A (WS VM) | Phase B (nospace interpreter) |
|------|-----------------|-------------------------------|
| 実行対象 | Whitespace 命令列 | nospace の Scope (意味解析結果) |
| ステップ粒度 | WS 命令単位 | 式評価単位 |
| デバッグ情報 | スタック・ヒープ・PC | 変数名・値・コールスタック・ソース位置 |
| 実装難易度 | 低（VM は既存） | 高（インタプリタの大規模改修） |
| 実行速度 | WS VM のオーバーヘッド | ネイティブインタプリタ |
| Web UI 適性 | 低レベルデバッガ向け | 高レベルデバッガ / Playground 向け |

**使い分け**:
- Phase A: Whitespace 命令レベルの動作を見たいとき、WS の学習ツールとして
- Phase B: nospace プログラムのデバッグ、Playground での実行・学習ツールとして

### 前提条件・依存タスク

| タスク | 状態 | 必要度 | 説明 |
|--------|------|--------|------|
| `suspendable-interpreter` | 未着手 | **必須** | インタプリタの中断・再開機能 |
| Phase 0（ビルド基盤） | 未着手 | 必須 | Cargo.toml + wasm-bindgen 設定 |
| `interpreter` ユニットテスト | 一部完了 | 推奨 | リファクタ前の動作保証 |

### suspendable-interpreter の要約

`suspendable-interpreter/` タスクで設計済みの **アプローチ B（Yield 伝播 + 継続保存）** を採用する。

現在のインタプリタは再帰呼び出しで実行状態を管理しており、中断・再開ができない。
`Flow::Yield` を追加し、ステップバジェット到達時に呼び出しチェーンを巻き戻して中断、
`Continuation` 情報で再開時に中断地点まで復帰する方式。

詳細は [../suspendable-interpreter/detailed-design.md](../suspendable-interpreter/detailed-design.md) を参照。

### Step B-1: suspendable-interpreter の実装

`suspendable-interpreter/` タスクの Phase 1〜4 を実施する。
これは Phase B の前提条件であり、最も工数が大きいパート。

| Phase | 内容 | API 公開への影響 |
|-------|------|-----------------|
| Phase 1 | 型と API の整備 (`InterpreterSession`, `StepResult`) | WASM ラッパーの型設計 |
| Phase 2 | Yield 導入（panic → Yield 返却） | 中断機能の基盤 |
| Phase 3 | 継続情報の保存・復元 | 再開機能の実現 |
| Phase 4 | テスト | 品質保証 |

タスク一覧:

- [ ] `InterpreterSession` / `StepResult` 型定義
- [ ] `Flow::Yield` の導入と伝播
- [ ] `Continuation` による状態保存・復元
- [ ] 既存テストがパスすることの確認
- [ ] ステップ実行のユニットテスト

### Step B-2: OwnedInterpreterSession の実装

`InterpreterSession<'a>` は `&'a Scope` を参照するが、WASM API ではオブジェクトの所有権を
JS 側に渡す必要がある。`Scope` を所有する新しいバリアントを実装する。

```rust
// src/interpreter/session.rs

/// Scope を所有するインタプリタセッション（WASM 用）
pub struct OwnedInterpreterSession {
    scope: Box<Scope>,
    env: Environment,
    continuation: Option<Continuation>,
    step_budget: usize,
}

impl OwnedInterpreterSession {
    pub fn new(scope: Scope, env: Environment, step_budget: usize) -> Self { ... }
    pub fn step(&mut self) -> StepResult { ... }
    pub fn env(&self) -> &Environment { ... }
    pub fn env_mut(&mut self) -> &mut Environment { ... }
}
```

タスク一覧:

- [ ] `OwnedInterpreterSession` 構造体の実装（Scope 所有版）
- [ ] `lib.rs` に公開 API 追加

### Step B-3: WasmInterpreterSession 型定義・コンストラクタ

```rust
// src/wasm_api.rs に追加

/// nospace インタプリタのステップ実行セッション
///
/// Scope と実行状態を保持し、step() 呼び出しで段階的に実行する。
#[wasm_bindgen]
pub struct WasmInterpreterSession {
    session: OwnedInterpreterSession,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}

#[wasm_bindgen]
impl WasmInterpreterSession {
    /// nospace ソースをパース・解析し、実行セッションを作成する
    ///
    /// step_budget: 1回の step() で実行する最大式評価回数
    #[wasm_bindgen(constructor)]
    pub fn new(
        source: &str,
        stdin: &str,
        step_budget: u32,
    ) -> Result<WasmInterpreterSession, JsValue>;
}
```

タスク一覧:

- [ ] `WasmInterpreterSession::new()` 実装
- [ ] `step()`, `is_complete()`, `expression_count()` 実装
- [ ] `flush_stdout()`, `get_traced()` 実装

### Step B-4: 実行制御メソッド

```rust
#[wasm_bindgen]
impl WasmInterpreterSession {
    /// step_budget 分だけ実行を進める
    ///
    /// 戻り値: {
    ///   status: "suspended" | "complete" | "error",
    ///   returnValue?: number,  // status="complete" 時
    ///   error?: string,        // status="error" 時
    /// }
    pub fn step(&mut self) -> JsValue;

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool;

    /// 総式評価回数
    pub fn expression_count(&self) -> usize;
}
```

### Step B-5: デバッグ情報 API

デバッグ情報の全てを初期実装に含める必要はない。以下の優先度で段階的に実装する：

| 優先度 | 機能 | 理由 |
|--------|------|------|
| 高 | `step()`, `is_complete()`, `flush_stdout()` | 最小限の実行機能 |
| 高 | `get_traced()` | テスト検証に必要 |
| 中 | `get_local_variables()`, `get_global_variables()` | Playground UI 向け |
| 中 | `get_call_stack()` | デバッガ UI 向け |
| 低 | ソース位置のハイライト | エディタ統合向け |

```rust
#[wasm_bindgen]
impl WasmInterpreterSession {
    /// 現在のスコープのローカル変数一覧
    ///
    /// 戻り値: [{ name: string, value: number }]
    pub fn get_local_variables(&self) -> JsValue;

    /// グローバル変数一覧
    ///
    /// 戻り値: [{ name: string, value: number }]
    pub fn get_global_variables(&self) -> JsValue;

    /// コールスタック（関数呼び出し履歴）
    ///
    /// 戻り値: [{ functionName: string, line?: number }]
    pub fn get_call_stack(&self) -> JsValue;

    /// 標準出力バッファの内容を取得しクリアする
    pub fn flush_stdout(&mut self) -> String;

    /// トレース情報を取得
    ///
    /// 戻り値: { [key: string]: number }
    pub fn get_traced(&self) -> JsValue;
}
```

デバッグ情報を提供するための内部実装:

```rust
// src/interpreter/session.rs

impl InterpreterSession<'_> {
    /// 現在のスコープのローカル変数を名前・値のペアで返す
    pub fn get_local_variables(&self) -> Vec<(String, i64)> {
        // continuation から現在の scope_depth / block を特定
        // scope の identifiers と local_env の values を突合
        todo!()
    }

    /// グローバル変数を名前・値のペアで返す
    pub fn get_global_variables(&self) -> Vec<(String, i64)> {
        // env.global_variables と scope.root_block.identifiers を突合
        todo!()
    }
}
```

タスク一覧:

- [ ] `get_local_variables()` 実装
- [ ] `get_global_variables()` 実装
- [ ] `get_call_stack()` 実装

### JS 側の利用例: Playground 実行

```javascript
import { WasmInterpreterSession } from './pkg/nospace20.js';

const source = `
func: main() {
  let: x;
  x = 0;
  while: x < 10 {
    __puti(x);
    __putc(10);
    x = x + 1;
  };
}
`;

const session = new WasmInterpreterSession(source, "", 5000);

function runChunk() {
  const result = session.step();
  
  const output = session.flush_stdout();
  if (output) {
    document.getElementById('output').textContent += output;
  }
  
  if (result.status === 'suspended') {
    requestAnimationFrame(runChunk);
  } else if (result.status === 'complete') {
    console.log('Done. Return:', result.returnValue);
  } else {
    console.error('Error:', result.error);
  }
}

runChunk();
```

### JS 側の利用例: デバッガ UI

```javascript
const session = new WasmInterpreterSession(source, stdin, 1);

function stepOne() {
  const result = session.step();
  
  updateVariablesPanel(session.get_local_variables());
  updateGlobalsPanel(session.get_global_variables());
  updateCallStackPanel(session.get_call_stack());
  updateExprCountDisplay(session.expression_count());
  
  const output = session.flush_stdout();
  if (output) appendOutput(output);
  
  return result;
}

document.getElementById('step-btn').onclick = stepOne;

document.getElementById('run-btn').onclick = () => {
  const batchSession = new WasmInterpreterSession(source, stdin, 100000);
  function chunk() {
    const result = batchSession.step();
    if (result.status === 'suspended') {
      const output = batchSession.flush_stdout();
      if (output) appendOutput(output);
      requestAnimationFrame(chunk);
    } else {
      const output = batchSession.flush_stdout();
      if (output) appendOutput(output);
      handleResult(result);
    }
  }
  chunk();
};
```

### Step B-6: テスト・検証

- [ ] Node.js でのスモークテスト（セッション作成 → step → 結果確認）
- [ ] 既存テストケースの WASM 経由実行・結果照合
- [ ] Phase A との結果一致確認

---

## Phase 3: テスト・統合

### Step 3-1: サイズ確認と最適化

```bash
# サイズ確認
ls -lh pkg/nospace20_bg.wasm

# wasm-opt による最適化
wasm-pack build --target nodejs --no-default-features --features wasm --release
```

### Step 3-2: 統合テスト

- [ ] Node.js でのスモークテスト（`run` / `compile` / ステップ実行の動作確認）
- [ ] 既存テストケースの一部を WASM 経由で実行し結果照合
- [ ] Phase A と Phase B の結果一致確認
- [ ] サイズ最適化（`wasm-opt`、不要機能の除外）

---

## 既存コードへの変更まとめ

| ファイル | 変更内容 | Phase |
|---------|---------|-------|
| `Cargo.toml` | `[lib]` セクション追加、`[features]` 追加（`cli`/`wasm`）、`clap`/`unicode-width` を optional 化、WASM 依存追加 | 0 |
| `.gitignore` | `/pkg/` 追加 | 0 |
| `src/lib.rs` | `#[cfg(feature = "wasm")] mod wasm_api;` 追加（1行） | 1 |
| `src/wasm_api.rs` | **新規作成**: run, compile, parse, WasmWhitespaceVM, WasmInterpreterSession | 1, A, B |
| `src/whitespace/interpreter.rs` | `pc()`, `call_stack_depth()` 等の軽微なメソッド追加 | A |
| `src/interpreter/session.rs` | **新規作成**: InterpreterSession, OwnedInterpreterSession | B |
| `src/interpreter/mod.rs` | Flow::Yield 追加、check_step_budget 改修 | B |

## 将来の改善

- `interpret_func_with_io` が `main` の戻り値を返すようにする
- 実行時間制限をデフォルトで設定（ブラウザ保護）
- `console_error_panic_hook` crate を導入し、wasm 内の panic をブラウザコンソールに出力
- WASM サイズの最適化（`wee_alloc` 等）
- TypeScript 型定義の手動補強（`wasm-pack` 自動生成 + 手書き `.d.ts`）
- `BigInt` 対応（64bit 整数のフルサポート）
