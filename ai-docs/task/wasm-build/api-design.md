# WASM 公開 API 設計

## 方針

CLI が提供する以下の機能を、JavaScript から呼び出し可能な関数として公開する:

| CLI 操作 | WASM API |
|----------|----------|
| `nospace20 source.ns` (`--mode=run`) | `run(source, stdin?, debug?)` |
| `nospace20 --mode=compile --target=ws --std=ws source.ns` | `compile(source, target, std?)` |
| パース → 意味解析 まで個別実行 | `parse(source)` （オプション） |

## 関数定義

### `run(source: string, stdin?: string, debug?: boolean) → RunResult`

nospace ソースコードを解析・実行し、結果を返す。
CLI の `--mode=run` に相当。

**パラメータ:**

| 名前 | 型 | デフォルト | 説明 |
|------|---|-----------|------|
| `source` | `string` | (必須) | nospace ソースコード |
| `stdin` | `string` | `""` | インタプリタへの標準入力 |
| `debug` | `boolean` | `false` | `true` の場合、trace 情報を含める |

**戻り値: `RunResult`**

```typescript
interface RunResult {
  success: true;
  returnValue: number | null;  // main 関数の戻り値
  stdout: string;              // 標準出力
  trace?: Record<string, string>;  // debug=true 時のみ、__trace() の記録
}
```

**エラー時:**

```typescript
interface ErrorResult {
  success: false;
  errors: Array<{
    message: string;
    line?: number;    // 1-based
    column?: number;  // 0-based
  }>;
}
```

### `compile(source: string, target: string, std?: string) → CompileResult`

nospace ソースコードを解析・コンパイルし、結果を返す。
CLI の `--mode=compile` に相当。

**パラメータ:**

| 名前 | 型 | デフォルト | 説明 |
|------|---|-----------|------|
| `source` | `string` | (必須) | nospace ソースコード |
| `target` | `string` | (必須) | `"ws"` / `"mnemonic"` |
| `std` | `string` | `"ws"` | `"standard"` / `"ws"` |

**バリデーション:** `target=ws/mnemonic` の場合は `std=ws` が必須（既存ルールを踏襲）。

**戻り値: `CompileResult`**

```typescript
interface CompileResult {
  success: true;
  output: string;  // コンパイル結果（Whitespace / ニーモニック）
}
```

エラー時は `ErrorResult` と同じ形式。

### `parse(source: string) → ParseResult`（オプション）

ソースコードの解析のみ行い、エラーの有無を返す。
エディタ統合・リアルタイムバリデーション向け。

**戻り値: `ParseResult`**

```typescript
interface ParseResult {
  success: true;
  // 必要に応じて AST 情報等を追加
}
```

エラー時は `ErrorResult` と同じ形式。

## JS/TS 型定義

wasm-pack は `.d.ts` を自動生成する。
`#[wasm_bindgen]` と `serde` を組み合わせて、上記の型が正しく生成されるようにする。

## Rust 側の型定義

```rust
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "success")]
pub enum WasmRunResult {
    #[serde(rename = "true")]
    Ok {
        #[serde(rename = "returnValue")]
        return_value: Option<i64>,
        stdout: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        trace: Option<std::collections::BTreeMap<String, String>>,
    },
    #[serde(rename = "false")]
    Err {
        errors: Vec<WasmError>,
    },
}

#[derive(Serialize)]
pub struct WasmError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}
```

`JsValue` への変換は `serde-wasm-bindgen` を使用:

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(source: &str, stdin: &str, debug: bool) -> JsValue {
    let result: WasmRunResult = run_internal(source, stdin, debug);
    serde_wasm_bindgen::to_value(&result).unwrap()
}
```

## エラーハンドリング

既存の `CodeParseError` をそのまま `WasmError` に変換する:

```rust
fn convert_errors(errors: &[CodeParseError], text: &TextCode) -> Vec<WasmError> {
    errors.iter().map(|e| {
        let (line, column) = e.code_pointer
            .map(|p| text.char_index_to_line(p))
            .unzip();
        WasmError {
            message: e.message.clone(),
            line,
            column,
        }
    }).collect()
}
```

## JS 側の使用例

### Node.js

```javascript
const nospace = require('./pkg/nospace20');

// 実行
const result = nospace.run('func:main(){__puti(42);}', '', false);
console.log(result);
// { success: true, returnValue: 0, stdout: "42" }

// コンパイル
const compiled = nospace.compile(
  'func:main(){__puti(42);}',
  'mnemonic',
  'ws'
);
console.log(compiled);
// { success: true, output: "push 42\ncall __puti\n..." }

// エラー
const bad = nospace.run('func:main(){', '', false);
console.log(bad);
// { success: false, errors: [{ message: "...", line: 1, column: 12 }] }
```

### ブラウザ (ES modules)

```javascript
import init, { run, compile } from './pkg/nospace20.js';

await init();

const result = run('func:main(){__puti(42);}', '', false);
```

## 制約・注意事項

- `i64` は JavaScript では `number` にマッピング（`wasm-bindgen` のデフォルト動作）。
  nospace の値域は i64 だが、JS の安全な整数範囲は ±2^53。
  大きな値を扱う場合は `BigInt` 対応が将来必要になる可能性がある。
- `__geti` / `__getc` は `stdin` パラメータ経由で入力を受け取る。
  対話的な入力（逐次読み込み）はサポートしない。
- 実行時間制限: `EnvironmentConfig::max_expression_count` を設定し、
  無限ループでブラウザがフリーズしないよう保護する（デフォルト値を設定）。
