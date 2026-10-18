# Version

## 2.0.1 (WIP)

- Parser: Fix infinite loop and unbounded memory use on an invalid token in a function call / struct literal argument list (e.g. `f(;`)
- whitespace20: Distinguish labels by their exact Space/Tab sequence (labels such as `ST` and `TT` no longer collide)
- whitespace20: Reject numbers outside the 64-bit signed range as a parse error instead of panicking, and parse `i64::MIN` correctly
- whitespace20: Arithmetic overflow now wraps (64-bit two's complement) instead of panicking or depending on the build profile
- Interpreter: Free scope memory on leaving a function/block so long loops no longer grow memory without bound
- `__putc` / OutputChar: Output 0-255 as a raw byte (was UTF-8 encoded Latin-1) and 256+ as a UTF-8 code point; invalid values are runtime errors
- Spec: State that the integer range and overflow behavior are implementation-dependent; describe this implementation's behavior in a new Appendix

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
