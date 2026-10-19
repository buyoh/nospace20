# Version

## 2.0.1 (WIP)

- Parser: Fix infinite loop and unbounded memory use on an invalid token in a function call / struct literal argument list (e.g. `f(;`)
- whitespace20: Distinguish labels by their exact Space/Tab sequence (labels such as `ST` and `TT` no longer collide)
- whitespace20: Reject numbers outside the 64-bit signed range as a parse error instead of panicking, and parse `i64::MIN` correctly
- whitespace20: Arithmetic overflow now wraps (64-bit two's complement) instead of panicking or depending on the build profile
- Interpreter: Free scope memory on leaving a function/block so long loops no longer grow memory without bound
- `__putc` / OutputChar: Output 0-255 as a raw byte (was UTF-8 encoded Latin-1) and 256+ as a UTF-8 code point; invalid values are runtime errors
- CI: Deploy script is split into build/push phases; GH_TOKEN is only passed to the push step, checkout credentials are not persisted, `npm ci --ignore-scripts`, and the webui ref can be pinned via GIT_REPO_WEBUI_REF
- Spec: State that the integer range and overflow behavior are implementation-dependent; describe this implementation's behavior in a new Appendix
- CLI: `--mode run` now executes on NospaceVM (explicit stack machine). Deep recursion no longer aborts with a stack overflow; runtime errors are reported as `error: ...` with exit code 1 instead of a panic
- Interpreter: NospaceVM reports runtime errors (division by zero, assertion failure, invalid memory access, invalid free, output errors such as broken pipe) as `StepResult::Error` instead of panicking; WASM no longer traps on them
- Interpreter: `__alloc` rejects negative sizes and sizes over 16777216 words as a runtime error instead of panicking / aborting on allocation failure
- Interpreter: NospaceVM limits the number of frames (default 1,000,000) and reports `stack overflow` for runaway recursion
- Interpreter: NospaceVM stays stopped after a runtime error (later `step()` calls return the same error) and honors `max_expression_count`; WASM `WasmNospaceVM.isFailed()` added
- Interpreter: NospaceVM reports assignment to a non-assignable left value as a runtime error instead of panicking
- Interpreter: NospaceVM returns no value (`main exited`) when `__main` ends without `return`, matching the tree-walking interpreter
- Parser: Limit syntax nesting depth to 128 (parentheses, unary/binary/postfix operator chains, type specs, blocks, `else: if:` chains, `repeat:`, ...) and report `nesting too deep` instead of overflowing the stack
- WASM: `compile()` now applies the peephole optimization for `["all"]` / `["peephole"]` (same output as the CLI); `getOptions().optPasses` includes `peephole`
- Optimizer: Pass names are defined once (`optimizer::PASS_NAMES` / `OptimizationOptions::from_pass_names`) and shared by the CLI and WASM

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
