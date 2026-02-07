# 実装手順

## Phase 1: ビルド基盤

### Step 1-1: Cargo.toml 変更

```toml
# 追加
[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = []
wasm = ["wasm-bindgen", "serde-wasm-bindgen"]

# [dependencies] に追加
wasm-bindgen = { version = "0.2", optional = true }
serde-wasm-bindgen = { version = "0.6", optional = true }
```

**確認**: `cargo build` / `cargo test` が従来通り成功すること。

### Step 1-2: .gitignore 更新

```
/pkg/
```

### Step 1-3: wasm32 コンパイル確認

```bash
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --lib --features wasm
```

この時点では `wasm_api.rs` はまだ空でよい。
ライブラリコード全体が wasm32 でコンパイルできることを確認する。

**潜在的な問題:**
- `std::io::stdin()` / `stdout()` → wasm32-unknown-unknown では no-op 実装が提供されるため、コンパイルは通る
- `build.rs` → ホスト側で実行されるため影響なし
- `clap` → `bin/` ターゲットのみで使用、`--lib` では含まれない

## Phase 2: WASM API 実装

### Step 2-1: wasm_api モジュール作成

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

### Step 2-2: lib.rs にモジュール登録

```rust
// src/lib.rs に追加
#[cfg(feature = "wasm")]
mod wasm_api;
```

### Step 2-3: 結果型の定義

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

### Step 2-4: エラー変換ヘルパー

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

### Step 2-5: `run` 関数実装

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

### Step 2-6: `compile` 関数実装

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

### Step 2-7: `parse` 関数実装（オプション）

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

## Phase 3: テスト・検証

### Step 3-1: wasm-pack build

```bash
wasm-pack build --target nodejs --features wasm
```

成功すれば `pkg/` に出力される。

### Step 3-2: Node.js スモークテスト

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

### Step 3-3: サイズ確認と最適化

```bash
# サイズ確認
ls -lh pkg/nospace20_bg.wasm

# wasm-opt による最適化（wasm-pack は自動で実行するがオプションで追加最適化可能）
wasm-pack build --target nodejs --features wasm --release
```

## 既存コードへの変更まとめ

| ファイル | 変更内容 |
|---------|---------|
| `Cargo.toml` | `[lib]` セクション追加、`[features]` 追加、依存追加 |
| `src/lib.rs` | `#[cfg(feature = "wasm")] mod wasm_api;` 追加（1行） |
| `src/wasm_api.rs` | **新規作成** |
| `.gitignore` | `/pkg/` 追加 |

既存のコードへの変更は最小限。新規ファイル `src/wasm_api.rs` が主な追加。

## 将来の改善

- `interpret_func_with_io` が `main` の戻り値を返すようにする
- 実行時間制限をデフォルトで設定（ブラウザ保護）
- `console_error_panic_hook` crate を導入し、wasm 内の panic をブラウザコンソールに出力
- WASM サイズの最適化（`wee_alloc` 等）
- TypeScript 型定義の手動補強（`wasm-pack` 自動生成 + 手書き `.d.ts`）
