# I/O インターフェース・ランタイム仕様

## 概要

nospace の組み込み関数（`__puti`, `__putc`, `__geti`, `__getc` 等）は、
インタプリタでは Rust のストリームを直接使用するが、
JS/WASM ターゲットではホスト環境が I/O を提供する必要がある。

本ドキュメントでは、JS/WASM コンパイラがホスト環境に期待するインターフェースを定義する。

## 設計方針

- コンパイラ出力は I/O 実装を**含まない**（インターフェースのみ定義）
- ホスト環境（別リポジトリ）が具体的な I/O 実装を注入する
- テスト用のデフォルト実装を同梱する（Node.js のコンソール I/O）

## ランタイムインターフェース

### 関数シグネチャ

| 関数名 | 引数 | 戻り値 | 説明 |
|--------|------|--------|------|
| `putInt(x)` | `i64` | `i64` (= x) | 整数 x を10進数文字列として出力 |
| `putChar(x)` | `i64` | `i64` (= x) | x を ASCII 文字として出力 |
| `getInt()` | なし | `i64` | 標準入力から整数を読み込み |
| `getChar()` | なし | `i64` | 標準入力から1文字を読み込み（ASCII 値） |

**重要**: `putInt` と `putChar` は引数の値をそのまま返す。
これは nospace の仕様で `__puti(x)` が式として使える（`y = __puti(x);` で y に x が入る）ため。

### 同期/非同期

- **出力関数** (`putInt`, `putChar`): 同期（即座に出力）
- **入力関数** (`getInt`, `getChar`): **同期を前提とする**

#### 入力の同期問題

ブラウザ環境では同期的な入力取得が困難（`prompt()` は UX が悪い）。

解決策の選択肢:

| 方式 | 説明 | メリット | デメリット |
|------|------|---------|-----------|
| **A: 事前入力** | 実行前に全入力を渡す | 実装が簡単 | 対話的でない |
| **B: SharedArrayBuffer** | Worker + Atomics で同期的にブロック | 完全な同期 I/O | ブラウザの COOP/COEP 設定必要 |
| **C: Asyncify** | WASM を中断・再開可能にする | 非同期ホストに対応 | 複雑、パフォーマンスコスト |
| **D: コルーチン変換** | コンパイラで CPS 変換 | WASM 単体で非同期対応 | コンパイラが非常に複雑に |

**推奨**: Phase 1 では **方式 A（事前入力）** を採用する。
別リポジトリ側で入力バッファを管理し、`getInt`/`getChar` はバッファから読み出す。

将来的に対話的実行が必要な場合は方式 B を検討する。

## JavaScript ターゲットのランタイム

### ランタイムオブジェクト

```typescript
interface NospaceRuntime {
  putInt(x: number): number;
  putChar(x: number): number;
  getInt(): number;
  getChar(): number;
}
```

### コンパイラ出力での使用

```javascript
// コンパイラが出力する JS コードの冒頭
const __nospace_runtime = (typeof __nospace_runtime_inject !== "undefined")
  ? __nospace_runtime_inject
  : {
      // デフォルト: Node.js 向け
      putInt: (x) => { process.stdout.write(String(x)); return x; },
      putChar: (x) => { process.stdout.write(String.fromCharCode(x)); return x; },
      getInt: () => 0,
      getChar: () => 0,
    };
```

### ホスト側の注入例（ブラウザ）

```javascript
// 別リポジトリ側
const inputBuffer = [];  // 事前に入力を設定
let inputPos = 0;
let outputText = "";

window.__nospace_runtime_inject = {
  putInt: (x) => { outputText += String(x); updateDisplay(); return x; },
  putChar: (x) => { outputText += String.fromCharCode(x); updateDisplay(); return x; },
  getInt: () => {
    // 入力バッファから読み取り
    while (inputPos < inputBuffer.length) {
      const ch = inputBuffer[inputPos];
      // 空白をスキップして数値を読み取る
      // ...
    }
    return 0;  // 入力なし
  },
  getChar: () => {
    if (inputPos < inputBuffer.length) {
      return inputBuffer[inputPos++].charCodeAt(0);
    }
    return 0;  // 入力なし（EOF）
  },
};
```

## WASM ターゲットのランタイム

### Import 仕様

WASM モジュールは以下のインポートを要求する:

```wat
(import "env" "putInt"  (func $__putInt  (param i64) (result i64)))
(import "env" "putChar" (func $__putChar (param i64) (result i64)))
(import "env" "getInt"  (func $__getInt  (result i64)))
(import "env" "getChar" (func $__getChar (result i64)))
```

- モジュール名: `"env"`
- 型: 全て `i64` ベース

### ホスト側のインスタンス化（JavaScript）

```javascript
const importObject = {
  env: {
    putInt: (x) => {
      outputText += String(x);
      return x;  // BigInt を返す
    },
    putChar: (x) => {
      outputText += String.fromCharCode(Number(x));
      return x;
    },
    getInt: () => {
      // ...
      return BigInt(0);
    },
    getChar: () => {
      // ...
      return BigInt(0);
    },
  },
};

const wasmModule = await WebAssembly.instantiate(wasmBytes, importObject);
```

**注意**: WASM の `i64` は JavaScript 側では `BigInt` として受け渡しされる。
ホスト側の I/O 実装は `BigInt` を適切に扱う必要がある。

## デバッグ用組み込み関数

### `__clog(x)`

| ターゲット | 動作 |
|-----------|------|
| JS | `console.log(x); return x;` |
| WASM | オプション: import として提供 or noop |

WASM で `__clog` をサポートする場合、追加のインポートが必要:

```wat
(import "env" "clog" (func $__clog (param i64) (result i64)))
```

### `__trace(key)`

全ターゲットで noop（何もしない）。戻り値は 0。

テスト用途では、ホスト側で trace をインポート関数として提供し、
テストフレームワーク側でカウンタを管理することも可能。

### `__assert(x)` / `__assert_not(x)`

| ターゲット | 動作 |
|-----------|------|
| JS | 条件不成立で `throw new Error(...)` |
| WASM | 条件不成立で `unreachable`（trap） |

## テスト用 I/O ハーネス

統合テストでは、I/O を検証するためのハーネスが必要。

### JS ターゲットのテストハーネス

```javascript
// test_harness.js - テスト実行スクリプト
const fs = require("fs");

const inputData = process.argv[2] || "";
let inputPos = 0;
let output = "";

global.__nospace_runtime_inject = {
  putInt: (x) => { output += String(x); return x; },
  putChar: (x) => { output += String.fromCharCode(x); return x; },
  getInt: () => {
    // 空白をスキップして整数を読み取る
    while (inputPos < inputData.length && /\s/.test(inputData[inputPos])) inputPos++;
    let numStr = "";
    while (inputPos < inputData.length && /[0-9-]/.test(inputData[inputPos])) {
      numStr += inputData[inputPos++];
    }
    return parseInt(numStr) || 0;
  },
  getChar: () => {
    if (inputPos < inputData.length) return inputData.charCodeAt(inputPos++);
    return 0;
  },
};

// コンパイル済みコードを実行
require(process.argv[3]);

// 出力を stdout に書き出し
process.stdout.write(output);
```

### WASM ターゲットのテストハーネス

```javascript
// test_harness_wasm.js
const fs = require("fs");

const wasmBytes = fs.readFileSync(process.argv[2]);
const inputData = process.argv[3] || "";
let inputPos = 0;
let output = "";

const importObject = {
  env: {
    putInt: (x) => { output += String(x); return x; },
    putChar: (x) => { output += String.fromCharCode(Number(x)); return x; },
    getInt: () => BigInt(0),
    getChar: () => BigInt(0),
  },
};

(async () => {
  const { instance } = await WebAssembly.instantiate(wasmBytes, importObject);
  // start セクションにより自動実行される
  // または明示的に: instance.exports.main();
  process.stdout.write(output);
})();
```

## 入力関数の詳細動作

### `getInt` の動作

nospace 仕様:
> `__geti` は空白・改行をスキップして数値を読み取る

具体的には:
1. 先頭の空白文字（`' '`, `'\t'`, `'\n'`, `'\r'`）をスキップ
2. オプションの符号（`-`）を読み取り
3. 連続する数字（`0-9`）を読み取り
4. 読み取った文字列を整数に変換
5. 入力が空（EOF）の場合は 0 を返す

### `getChar` の動作

nospace 仕様:
> `__getc` は次の1バイトをそのまま読み取る（空白・改行もそのまま）

具体的には:
1. 次の1バイトを読み取り
2. バイト値（0-255）をそのまま返す
3. 入力が空（EOF）の場合は 0 を返す（実装依存）

## 将来の拡張

### 非同期 I/O

対話的な実行が必要になった場合:

1. JS ターゲット: `async/await` ベースに変換（コンパイラで CPS 変換）
2. WASM ターゲット: Asyncify または SharedArrayBuffer + Worker

### ファイル I/O

将来的に nospace にファイル操作が追加された場合、ランタイムインターフェースを拡張する。

### カスタム組み込み関数

ホスト側で独自の組み込み関数を追加できるよう、可変長のインポートテーブルを検討する。
