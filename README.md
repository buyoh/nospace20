# Nospace20

## What is Nospace20?

Nospace is a toy programming language that allows arbitrary spaces, newlines, and tabs anywhere in the code, inspired by the esoteric programming language Whitespace. Nospace20 compiles Nospace code into Whitespace, and can also run it directly (run mode compiles to Whitespace and executes it on the built-in Whitespace VM).

Nospace とは、改行、タブ・半角スペース等の空白に影響を受けることなく記述できるプログラミング言語です。esolang である Whitespace と対になる言語を目指しています。Nospace20 は Whitespace へのコンパイラで、run モードではコンパイルした Whitespace を内蔵の Whitespace VM で実行します。

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

`nospace20` compiles nospace to Whitespace, and `whitespace20` runs Whitespace on the built-in VM.

Compile and run:

```bash
$ cargo build --release
$ target/release/nospace20 resources/tests/passes/examples/e0-01-fibonacci.ns \
    --std-ext debug --opt all -o /tmp/out.ws
$ echo 10 | target/release/whitespace20 --std-ext debug /tmp/out.ws
89
```

Pass the same `--std-ext` to both commands: the debug extension (`__assert`, `__assert_not`, `__trace`, `__clog`) works only when both the compiler and the VM enable it (without it, debug built-ins are ignored, but their arguments are still evaluated). `__clog` writes to stderr.
The alloc extension is enabled by default; `--no-std-ext` disables it.
`whitespace20` limits execution to 16777216 memory cells by default (`--max-memory-cells`, `0` = unlimited), so infinite recursion stops with `memory limit exceeded`.
`whitespace20 -` reads the Whitespace source from stdin; the program's input is then given by `--input FILE`.

Run a nospace program instantly (compile and run without temporary files; requires bash and `/dev/fd`, i.e. Linux or macOS):

```bash
$ echo 10 | NOSPACE20=target/release/nospace20 WHITESPACE20=target/release/whitespace20 \
    tools/nsrun.sh --opt all resources/tests/passes/examples/e0-01-fibonacci.ns
89
```

`tools/nsrun.sh [--ignore-debug] [NOSPACE20_ARGS...] FILE.ns [-- WHITESPACE20_ARGS...]` enables the debug extension unless `--ignore-debug`, passes arguments before `--` to `nospace20` and after `--` to `whitespace20`. `NOSPACE20` / `WHITESPACE20` select the binaries (default: `nospace20` / `whitespace20` on `PATH`; `cargo install` does not install the script).

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
wasm-pack build --dev --out-dir pkg --target bundler --features wasm

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
