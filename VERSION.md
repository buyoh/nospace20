# Version

## x.x.x (WIP)

MAJOR

- CLI: Remove the direct interpreter and `--mode` / `--ignore-debug` from nospace20. nospace20 is now a compiler only: `nospace20 x.ns` writes Whitespace to stdout (previously it ran the program). Run the output with whitespace20, passing the same `--std-ext` to both (e.g. `nospace20 --std-ext debug x.ns -o x.ws && whitespace20 --std-ext debug x.ws`), or use `tools/nsrun.sh x.ns` to compile and run at once (the debug extension is enabled unless `--ignore-debug`). Compared with the former run mode: runtime error messages are those of the Whitespace VM, the `main returns: N` / `main exited` lines are no longer printed, misuse of `__alloc` / `__free` (freed or out-of-range access, invalid free, invalid size) is no longer detected, using `__alloc` / `__free` with `--no-std-ext` is a compile error, and `--debug` trace results are written by `whitespace20 --debug` to stderr
- CLI: nospace20 `--debug` only prints the output file name; a missing `__main` is reported as `__main function not found`
- whitespace20: The memory limit is 16,777,216 cells by default (was unlimited; `--max-memory-cells 0` for unlimited)
- Whitespace VM / whitespace20 / WASM: `__geti` (InputNumber) reads one line and parses it like wsc: surrounding Unicode whitespace is trimmed, an optional `+` / `-` sign, decimal digits, and `_` separators after the first digit (`1_000`). Empty lines, more than one number on a line and values outside i64 stop with the new runtime error `invalid number input: "<line>"`; end of input stops with `I/O error: unexpected end of input`. Interactive stdin (WASM) now decodes the line as UTF-8 instead of Latin-1
- Library: Add `WsRuntimeError::InvalidNumberInput`. Remove `ExecutionMode`, `CompileProperty::mode`, `CompileProperty::ignore_debug` and `ValidationError::CompileOnlyOption`
- WASM: Remove `WasmNospaceVM`. Use `WasmWhitespaceVM` (constructed from nospace source) instead; pass `["debug", "alloc"]` as `std_extensions` for the previous default (`["alloc"]` for the previous `ignore_debug`). `getReturnValue()` / `isFailed()` have no replacement, `flushStdout()` / `getTraced()` are `flush_stdout()` / `get_traced()`, and `total_steps()` counts Whitespace instructions
- Library: Remove the direct interpreter API (`interpret`, `interpret_with_env`, `interpret_func`, `interpret_func_with_env`, `interpret_func_testing`, `interpret_func_testing_randomize`, `interpret_func_with_io`, `Environment`, `EnvironmentConfig`, `InterpretError`, `NospaceVM`, `StepResult`, `NospaceError::Interpret`, `CompileStage::NospaceExecution`) and `algorithm::hash::lcg_hash_with_offset`

MINOR

- whitespace20: Accept `-` as the source file (read the Whitespace source from stdin; give the program's input with `--input`)
- Library: Add `analyze_source` / `compile_source` (nospace source to `Scope` / Whitespace, shared by the CLI, WASM and tests), the `runner` module (`build_vm`, `run_vm_to_end`, `run_ws_to_end`, `WsRunOptions`, `RunIo`, `WsRunOutcome`, `WsRunError`), `WhitespaceVM::try_flush` and the `SharedWriter` re-export
- Tools: Add `tools/nsrun.sh`, which compiles a nospace program and runs it on whitespace20 without temporary files

PATCH

- whitespace20: A failure to flush stdout at exit (e.g. `> /dev/full`) is now reported with exit code 1 (was exit 0)
- WASM `compile` / `WasmWhitespaceVM::new`: An invalid optimization pass name is reported before syntax errors in the source
- nospace20: A failure to write the output to stdout (e.g. `> /dev/full`, closed pipe) is now reported with exit code 1 (was a panic or exit 0)

## 3.2.0

MINOR

- Whitespace VM / whitespace20: Add an optional memory limit counted in cells (heap addresses + data stack + return addresses). whitespace20 `--max-memory-cells` (unlimited by default); stops with `memory limit exceeded: N cells`
- Whitespace VM / compiler: Add the debug extension API `-13` for `__clog`. With `--std-ext debug`, `__clog(x)` writes `__clog: x` to stderr in whitespace20 (separate from the program's stdout; discarded in WASM). Without the debug extension, `__clog` is still a no-op in Whitespace
- WASM `WasmWhitespaceVM::new`: Accept `opt_passes` and `max_memory_cells`
- WASM `WasmWhitespaceVM`: Runtime errors from `step()` use the display message (e.g. `division by zero` instead of `DivisionByZero`)
- WASM `WasmWhitespaceVM::new`: A memory limit of 4,194,304 cells is now applied by default (pass `max_memory_cells: 0` for unlimited). `fromWhitespace` / `fromWhitespaceInteractive` stay unlimited

PATCH

- WASM `WasmWhitespaceVM::new`: The debug extension passed in `std_extensions` is now applied to the VM (previously `__trace` was not recorded and `__assert` never failed). Programs that pass `["debug"]` may now stop with an assertion error
- whitespace20 / Whitespace VM: InputChar / InputNumber now write the heap through the same path as Store (debug extension API addresses apply to input, and profiler heap statistics `heap_store_*` now include input writes)

## 3.1.0

MINOR

- Language: Functions can be declared in block scopes (`{}`, `if` / `while` / `for` / `repeat` bodies, block aliases). They are visible only inside the block and cannot read non-static variables of enclosing blocks (block `static` variables are not accessible yet)

PATCH

- Semantic analyzer: Fix an internal panic when a function is declared inside a function in a namespace
- Semantic analyzer: A nested function no longer takes the final flag from a same-named non-static variable of its enclosing function
- Semantic analyzer / compiler: Report an error instead of panicking or silently wrapping when the size of arrays, structs or variables overflows
- Compiler: Fix return / break / continue inside an expression (e.g. a block expression) leaving intermediate values on the stack

## 3.0.0

Various bug fixes and improvements

MAJOR

- Interpreter: NospaceVM stays stopped after a runtime error (later `step()` calls return the same error) and honors `max_expression_count`; WASM `WasmNospaceVM.isFailed()` added
- WASM: `compile()` now applies the peephole optimization for `["all"]` / `["peephole"]` (same output as the CLI); `getOptions().optPasses` includes `peephole`
- Optimizer: Pass names are defined once (`optimizer::PASS_NAMES` / `OptimizationOptions::from_pass_names`) and shared by the CLI and WASM

MINOR

- CLI: Compile errors are printed to stderr (were printed to stdout)
- CLI: Fail when stdin cannot be read (e.g. invalid UTF-8) instead of treating it as an empty source
- CLI: `--version` shows the package version (was fixed at 0.1.0)
- CLI: `--target` / `--output` with `--mode run` is an error (was silently ignored). `CompileProperty.target` is now `Option<CompileTarget>`
- whitespace20: Print parse and runtime errors with their readable messages
- CLI: `--mode run` now executes on NospaceVM (explicit stack machine). Deep recursion no longer aborts with a stack overflow; runtime errors are reported as `error: ...` with exit code 1 instead of a panic
- `__putc` / OutputChar: Output 0-255 as a raw byte (was UTF-8 encoded Latin-1) and 256+ as a UTF-8 code point; invalid values are runtime errors

PATCH

- Parser: Fix infinite loop and unbounded memory use on an invalid token in a function call / struct literal argument list (e.g. `f(;`)
- Interpreter: Static variables are accessed directly in their static storage instead of being copied into each call frame. Updates made during recursion are no longer lost, pointers to static variables stay valid across calls, recursive nested functions update the parent's statics correctly, and `__main` statics keep their initializer value (tree-walking interpreter)
- Interpreter: NospaceVM no longer corrupts variable resolution after `return` from inside a `for` loop
- whitespace20: Distinguish labels by their exact Space/Tab sequence (labels such as `ST` and `TT` no longer collide)
- whitespace20: Reject numbers outside the 64-bit signed range as a parse error instead of panicking, and parse `i64::MIN` correctly
- whitespace20: Arithmetic overflow now wraps (64-bit two's complement) instead of panicking or depending on the build profile
- Interpreter: Free scope memory on leaving a function/block so long loops no longer grow memory without bound
- Spec: State that the integer range and overflow behavior are implementation-dependent; describe this implementation's behavior in a new Appendix
- Interpreter: NospaceVM reports runtime errors (division by zero, assertion failure, invalid memory access, invalid free, output errors such as broken pipe) as `StepResult::Error` instead of panicking; WASM no longer traps on them
- Interpreter: `__alloc` rejects negative sizes and sizes over 16777216 words as a runtime error instead of panicking / aborting on allocation failure
- Interpreter: NospaceVM limits the number of frames (default 1,000,000) and reports `stack overflow` for runaway recursion
- Interpreter: NospaceVM reports assignment to a non-assignable left value as a runtime error instead of panicking
- Interpreter: NospaceVM returns no value (`main exited`) when `__main` ends without `return`, matching the tree-walking interpreter
- Parser: Limit syntax nesting depth to 128 (parentheses, unary/binary/postfix operator chains, type specs, blocks, `else: if:` chains, `repeat:`, ...) and report `nesting too deep` instead of overflowing the stack
- WS compiler: Fix variables in root-level blocks and in function static initializers being placed at the same addresses as global/static variables or the allocator area; fix static variables accessed from nested blocks in a function resolving to a wrong address
- Semantic analyzer: Reject function declarations in block scopes (`{}`, `if` / `while` / `for` / `repeat` bodies, block aliases) as a compile error. They were previously accepted and calls resolved to an unrelated function (inside a namespaced function, they caused an internal panic)

## 2.0.0

tag: v2.0.0

- Lang: Fix array postfix-subscript desugaring rule to `*(&expr + i)`
  - Previous version desugared `expr[i]` to `*(expr + i)`, which assumed `expr` was a pointer. The new rule `*(&expr + i)` correctly handles both array names and pointer expressions.

## 1.2.0

tag: v1.2.0

- Add `import:` feature that is similar to `using namespace` in C++ and `include` in Ruby.

## 1.1.1

tag: "N/A"

- Lang: Fix array type acceptance in semantic analysis
  - An array element was not able to be used in value context.

## 1.1.0

tag: v1.1.0

- Lang: Add `namespace` feature
- Lang: Add explicit type system
- Lang: Add `struct` feature
- Other: Add examples
  - `e2-00-shortest-path`
  - `e1-02-union-find`

## 1.0.0

tag: v1.0.0

## Semver

- MAJOR version is incremented when there are incompatible API changes.
- MINOR version is incremented when functionality is added in a backwards-compatible manner.
- PATCH version is incremented when backwards-compatible bug fixes are made.
