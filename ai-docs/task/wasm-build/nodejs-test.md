# Node.js WASM テスト設計

## 概要

`wasm-pack build` で生成された WASM モジュールを Node.js 環境で動作確認するテストを実装する。
Phase 3（テスト・統合）の一部として、すべての公開 API が正しく動作することを検証する。

## テスト対象 API

現在 `src/wasm_api.rs` で公開されている API:

### 基本関数

| 関数 | 説明 |
|------|------|
| `run(source, stdin, debug)` | nospace コードの実行 |
| `compile(source, target, lang_std)` | nospace → Whitespace/mnemonic コンパイル |
| `parse(source)` | 構文チェックのみ |
| `compile_to_whitespace_string(source)` | ヘルパー（`compile()` のラッパー） |
| `compile_to_mnemonic_string(source)` | ヘルパー（`compile()` のラッパー） |

### WasmWhitespaceVM クラス

| メソッド | 説明 |
|----------|------|
| `new(nospace_source, stdin)` | nospace からコンパイルして VM 構築 |
| `fromWhitespace(ws_source, stdin)` | Whitespace から VM 構築 |
| `step(budget)` | 指定ステップ数だけ実行 |
| `pc()` | プログラムカウンタ |
| `total_steps()` | 総実行命令数 |
| `is_complete()` | 実行完了判定 |
| `get_stack()` | データスタック取得 |
| `get_heap()` | ヒープ取得 |
| `call_stack_depth()` | コールスタック深さ |
| `flush_stdout()` | 標準出力取得・クリア |
| `get_traced()` | トレース情報取得 |
| `current_instruction()` | 現在命令のニーモニック |
| `disassemble()` | 命令列全体のニーモニック |

## テスト環境のセットアップ

### 前提条件

- Node.js (v18 以上推奨、ES modules サポート)
- wasm-pack (`cargo install wasm-pack` または `npx wasm-pack`)

Node.js はテスト実行にのみ必要で、`wasm-pack build` 自体は Node.js に依存しない。

### ビルド手順

```bash
# WASM ビルド（Node.js ターゲット）
wasm-pack build --target nodejs --no-default-features --features wasm

# 出力先: pkg/
# - nospace20.js       (CommonJS エントリポイント)
# - nospace20_bg.wasm  (WASM バイナリ)
# - nospace20.d.ts     (TypeScript 型定義)
```

### ディレクトリ構成

```
nospace20/
├── pkg/                      # wasm-pack 出力（.gitignore で除外）
│   ├── nospace20.js
│   ├── nospace20_bg.wasm
│   └── nospace20.d.ts
├── tools/
│   └── wasm-test/            # WASM テストディレクトリ（新規）
│       ├── package.json      # テスト用 npm 設定
│       └── test.mjs          # テストスクリプト
```

## テストスクリプト設計

### 構成

`tools/wasm-test/test.mjs` で全テストを実行する単一ファイル構成。
外部テストフレームワーク（Jest, Mocha 等）を使わず、シンプルな assert ベースで実装。

### テストケース

#### 1. run() 関数テスト

```javascript
// 1.1 基本実行
const result = run('func main() { __println(42); }', '', false);
assert(result.success === true);
assert(result.stdout === '42\n');

// 1.2 stdin の利用
const result2 = run('func main() { let: x; __readInt(x); __println(x); }', '123', false);
assert(result2.stdout === '123\n');

// 1.3 debug モード（trace 出力）
const result3 = run('func main() { let: x; x = 10; __trace(x); }', '', true);
assert(result3.trace !== undefined);

// 1.4 構文エラー
const result4 = run('func main() { }', '', false);  // 不正なコード
assert(result4.success === false);
assert(result4.errors.length > 0);
```

#### 2. compile() 関数テスト

```javascript
// 2.1 Whitespace へのコンパイル
const result = compile('func main() { __println(1); }', 'ws', 'ws');
assert(result.success === true);
assert(typeof result.output === 'string');
assert(result.output.length > 0);

// 2.2 mnemonic へのコンパイル
const result2 = compile('func main() { __println(1); }', 'mnemonic', 'ws');
assert(result2.success === true);
assert(result2.output.includes('push'));

// 2.3 無効なターゲット
const result3 = compile('func main() {}', 'invalid', 'ws');
assert(result3.success === false);

// 2.4 std 不一致エラー
const result4 = compile('func main() {}', 'ws', 'standard');
assert(result4.success === false);
```

#### 3. parse() 関数テスト

```javascript
// 3.1 正常なコード
const result = parse('func main() { let: x; x = 1; }');
assert(result.success === true);

// 3.2 構文エラー
const result2 = parse('func main() { let x; }'); // コロン不足
assert(result2.success === false);
```

#### 4. WasmWhitespaceVM テスト

```javascript
// 4.1 コンストラクタ
const vm = new WasmWhitespaceVM('func main() { __println(1); }', '');
assert(vm.is_complete() === false);

// 4.2 ステップ実行
let stepResult = vm.step(1000);
assert(['suspended', 'complete'].includes(stepResult.status));

// 4.3 完了まで実行
while (!vm.is_complete()) {
  vm.step(100);
}
assert(vm.is_complete() === true);

// 4.4 stdout 取得
const stdout = vm.flush_stdout();
assert(stdout === '1\n');

// 4.5 スタック・ヒープ取得
const stack = vm.get_stack();
assert(Array.isArray(stack));
const heap = vm.get_heap();
assert(typeof heap === 'object');

// 4.6 fromWhitespace コンストラクタ
const compiled = compile('func main() { __println(2); }', 'ws', 'ws');
const vm2 = WasmWhitespaceVM.fromWhitespace(compiled.output, '');
while (!vm2.is_complete()) {
  vm2.step(100);
}
assert(vm2.flush_stdout() === '2\n');
```

### テスト実行方法

```bash
# テスト実行（プロジェクトルートから）
cd tools/wasm-test && node test.mjs
```

### package.json

```json
{
  "name": "nospace20-wasm-test",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "test": "node test.mjs"
  }
}
```

## 実装手順

### Phase 1: テスト環境構築

- [x] `tools/wasm-test/` ディレクトリ作成
- [x] `tools/wasm-test/package.json` 作成

### Phase 2: テストスクリプト実装

- [x] `tools/wasm-test/test.mjs` 作成
- [x] 基本関数テスト実装（run, compile, parse）
- [x] WasmWhitespaceVM テスト実装
- [x] エラーケーステスト実装

### Phase 3: 検証・統合

- [ ] 全テスト通過確認（wasm-pack 未導入のため未実施）
- [x] README.md にテスト実行方法を追記

失敗調査メモ: [nodejs-test-failure.md](nodejs-test-failure.md)

## 備考

### i64 と JavaScript Number の精度

JavaScript の Number は IEEE 754 倍精度浮動小数点数であり、整数として正確に表現できる範囲は `±2^53` まで。
`get_stack()` や `get_heap()` で返される値はこの制限を受ける。

将来、大きな整数を扱う必要がある場合は `get_stack_bigint()` などの追加が必要。

### テストの独立性

テストは外部テストフレームワークに依存せず、Node.js の組み込み `assert` モジュールのみを使用。
CI 環境でも追加の依存関係なしで実行可能。

## 関連ドキュメント

- [README.md](README.md) - WASM ビルドタスク概要
- [api-design.md](api-design.md) - WASM API 設計
- [implementation.md](implementation.md) - 実装詳細
