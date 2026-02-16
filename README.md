# Nospace20

## What is Nospace20?

Nospace is a toy programming language that allows arbitrary spaces, newlines, and tabs anywhere in the code, inspired by the esoteric programming language Whitespace. Nospace20 can interpret Nospace code. ~~It can also compile Nospace code into Whitespace.~~

Nospace とは、改行、タブ・半角スペース等の空白に影響を受けることなく記述できるプログラミング言語です。esolang である whitespace と対になる言語を目指しています。Nospace20 は interpreter として動作する他、~~whitespace へのコンパイルも可能です~~。

```
func: puts(str) {
  while: *str != 0 {
    __putc(*str);
    str += 1;
  };
  __putc('\n');
}

func: main() {
  let: g[12]("hello\sworld");
  puts(&g);
  return: 0;
}
```

## CLI Usage

```bash
cargo run --bin nospace20
cargo run --release --bin nospace20
```

```
A nospace language interpreter and compiler

Usage: nospace20 [OPTIONS] [FILE]

Arguments:
  [FILE]  Source file to execute (reads from stdin if not provided)

Options:
      --std <STD>        Language subset [default: standard] [possible values: standard, min, ws]
      --mode <MODE>      Execution mode [default: run] [possible values: run, compile]
      --target <TARGET>  Compile target (only with --mode=compile) [default: ws] [possible values: ws, mnemonic, ex-ws, json]
  -o, --output <OUTPUT>  Output file (only with --mode=compile, stdout if not specified)
  -d, --debug            Show trace results after execution
  -h, --help             Print help
  -V, --version          Print version
```

- `--std`
  - 言語のサブセットを指定するために使う。
    - `standard` : 全ての機能が有効。デフォルト。
    - `min` : （未対応）最小限の機能セット。
    - `ws` : whitespace へのコンパイル時に選択。
  - 用途は以下の通り
    - `standard` : 全ての機能が有効。デフォルト。
    - `min` : セルフホスティングコンパイラを構築する際に使用。
    - `ws` : 例えば bit 演算等は whitespace では実装できないため。
- `--mode`
  - `run`  : インタプリタモード。直接実行する。デフォルト。
  - `compile` : コンパイルモード。
- `--target`
  - `ws` : whitespace へコンパイル。`std` が `ws` の場合のみ。
  - `mnemonic`: ニーモニック表記へコンパイル。`std` が `ws` の場合のみ。
  - `ex-ws` : （未対応）拡張 whitespace へコンパイル。
  - `json` : （未対応）意味解析後の中間表現へコンパイル。

## Build

### Standard Build (CLI)

```bash
cargo build
cargo build --release
```

### WebAssembly Build

```bash
# Add wasm32 target (first time only)
rustup target add wasm32-unknown-unknown

# Build for WebAssembly
cargo build --target wasm32-unknown-unknown --lib --no-default-features --features wasm
```

### WASM Node.js Tests

```bash
# Build WASM (bundler target works with Node.js)
wasm-pack build --target bundler --features wasm
# Debug build
# wasm-pack build --dev  --out-dir pkg-dev --target bundler --features wasm

# Note: Node.js is only required to run the tests. The wasm-pack build itself does not depend on Node.js.

# Run Node.js smoke tests
cd tools/wasm-test && node test.mjs
```

### Feature Flags

- `cli` (default): Enable CLI binary build with `clap` and `unicode-width` dependencies
- `wasm`: Enable WebAssembly build with `wasm-bindgen` and `serde-wasm-bindgen` dependencies

### Tests

```bash
cargo test
# wsc only supports x64, not for arm64
bash tools/setup-wsc.sh  # Once to setup wsc (Whitespace Compiler)
cargo test -- --ignored
```

## Other Features

### Whitespace20

TODO:

## docs

TODO: English docs

- [spec.md](./spec.md) : nospace language specification
- [tutorial.md](./tutorial.md) : A simple tutorial
