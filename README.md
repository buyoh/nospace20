# Nospace20

## What is Nospace20?

Nospace is a toy programming language that allows arbitrary spaces, newlines, and tabs anywhere in the code, inspired by the esoteric programming language Whitespace. Nospace20 can interpret Nospace code. It can also compile Nospace code into Whitespace.

Nospace とは、改行、タブ・半角スペース等の空白に影響を受けることなく記述できるプログラミング言語です。esolang である Whitespace と対になる言語を目指しています。Nospace20 は interpreter として動作する他、Whitespace へのコンパイルも可能です。

```
func: puts(str) {
  while: *str != 0 {
    __putc(*str);
    str += 1;
  };
  __putc('\n');
}

func: __main() {
  let: g[12]("hello\sworld");
  puts(&g);
  return: 0;
}
```

If you want to see more examples, please check the [test cases](./resources/tests/passes/examples).

## Web Editor

You can try Nospace20 in the browser! https://buyoh.github.io/nospace20/

Web Editor is implemented as a separate project, https://github.com/buyoh/nospace20-webui .

## CLI Usage

Run Nospace20 instantly.

```bash
$ echo 10 | cargo run --release --bin nospace20 -- \
    resources/tests/passes/examples/e0-01-fibonacci.ns --std-ext debug --opt all 
89
main exited
```

Compile and run:

```bash
$ cargo run --bin nospace20 -- \
    resources/tests/passes/examples/e0-01-fibonacci.ns \
    --std-ext debug --opt all --mode compile --target ws > /tmp/out.ws
$ echo 10 | cargo run --bin whitespace20 -- /tmp/out.ws
89
```

## Build

### Feature Flags

- `cli` (default): Enable CLI binary build with `clap` and `unicode-width` dependencies
- `wasm`: Enable WebAssembly build with `wasm-bindgen` and `serde-wasm-bindgen` dependencies

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
wasm-pack build --dev --out-dir pkg-dev --target bundler --features wasm

# Note: Node.js is only required to run the tests. The wasm-pack build itself does not depend on Node.js.
# Run Node.js smoke tests
cd tools/wasm-test && node test.mjs
```

## Tests

```bash
cargo test
# wsc only supports x64, not for arm64
bash tools/setup-wsc.sh  # Once to setup wsc (Whitespace Compiler)
cargo test -- --ignored
```

## docs

TODO: English docs

- [docs/spec.md](./docs/spec.md) : nospace language specification
- [docs/tutorial.md](./docs/tutorial.md) : A simple tutorial
