import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkgDir = join(__dirname, "../../pkg");

// WASM ファイルを読み込んで初期化 (bundler target 対応)
const wasmPath = join(pkgDir, "nospace20_bg.wasm");
const wasmBytes = await readFile(wasmPath);

// nospace20_bg.js をインポート
const bg = await import("../../pkg/nospace20_bg.js");

// WASM を初期化
const wasmModule = await WebAssembly.compile(wasmBytes);
const imports = {
  "./nospace20_bg.js": bg,
};
const wasmInstance = await WebAssembly.instantiate(wasmModule, imports);

// wasm を設定
bg.__wbg_set_wasm(wasmInstance.exports);

// 初期化関数を実行
if (wasmInstance.exports.__wbindgen_start) {
  wasmInstance.exports.__wbindgen_start();
}

// 使用する関数を取得
const {
  compile,
  parse,
  WasmWhitespaceVM,
  WasmNospaceVM,
} = bg;

function expectSuccess(result, label) {
  assert.equal(result.success, true, `${label}: expected success`);
}

function expectFailure(result, label) {
  assert.equal(result.success, false, `${label}: expected failure`);
  assert.ok(Array.isArray(result.errors), `${label}: expected errors array`);
  assert.ok(result.errors.length > 0, `${label}: expected errors`);
}

function runToComplete(vm, budget = 100, maxIterations = 1000) {
  for (let i = 0; i < maxIterations; i += 1) {
    const stepResult = vm.step(budget);
    assert.ok(
      ["suspended", "complete", "error"].includes(stepResult.status),
      "step(): unexpected status"
    );
    if (stepResult.status === "error") {
      throw new Error(`vm step error: ${stepResult.error}`);
    }
    if (vm.is_complete()) {
      return;
    }
  }
  throw new Error("vm did not complete within iteration limit");
}

// 1. WasmNospaceVM tests (replaces run() API)
{
  // Basic execution
  const vm1 = new WasmNospaceVM("func: __main() { __puti(42); __putc(10); }", "");
  runToComplete(vm1, 1000, 1000);
  assert.equal(vm1.is_complete(), true, "vm1 completion");
  const stdout1 = vm1.flushStdout();
  assert.equal(stdout1, "42\n", "vm1 stdout");

  // stdin support
  const vm2 = new WasmNospaceVM(
    "func: __main() { let: x; x = __geti(); __puti(x); __putc(10); }",
    "123\n"
  );
  runToComplete(vm2, 1000, 1000);
  assert.equal(vm2.flushStdout(), "123\n", "vm2 stdin stdout");

  // trace support
  const vm3 = new WasmNospaceVM("func: __main() { let: x; x = 10; __trace(x); }", "");
  runToComplete(vm3, 1000, 1000);
  const traced = vm3.getTraced();
  assert.equal(typeof traced, "object", "vm3 traced type");
  assert.equal(traced["10"], 1, "vm3 traced value");

  // step-by-step execution (suspension)
  const vm4 = new WasmNospaceVM("func: __main() { __puti(1); __putc(10); }", "");
  let stepCount = 0;
  while (!vm4.is_complete()) {
    const result = vm4.step(1);
    assert.ok(
      ["suspended", "complete"].includes(result.status),
      "vm4 step status"
    );
    stepCount++;
    if (stepCount > 10000) throw new Error("vm4 did not complete");
  }
  assert.ok(stepCount > 1, "vm4 took multiple steps");
  assert.equal(vm4.flushStdout(), "1\n", "vm4 stdout");

  // total_steps
  assert.ok(vm4.total_steps() > 0, "vm4 total_steps > 0");

  // return value
  const vm5 = new WasmNospaceVM("func: __main() { return: 42; }", "");
  runToComplete(vm5, 1000, 1000);
  assert.equal(vm5.getReturnValue(), 42n, "vm5 return value");

  // ignore_debug option
  const vm6 = new WasmNospaceVM(
    "func: __main() { __assert(0); __puti(1); __putc(10); }",
    "",
    undefined,
    true
  );
  runToComplete(vm6, 1000, 1000);
  assert.equal(vm6.flushStdout(), "1\n", "vm6 ignore_debug stdout");

  // opt_passes option
  const vm7 = new WasmNospaceVM(
    "func: __main() { __puti(1 + 2); __putc(10); }",
    "",
    ["constant-folding"]
  );
  runToComplete(vm7, 1000, 1000);
  assert.equal(vm7.flushStdout(), "3\n", "vm7 opt_passes stdout");

  // syntax error
  try {
    new WasmNospaceVM("func: __main() { let x; }", "");
    assert.fail("should throw on syntax error");
  } catch (e) {
    // Expected: wasm-bindgen throws JsValue on error
    assert.ok(true, "vm syntax error");
  }
}

// 2. compile() tests
{
  const result = compile("func: __main() { __puti(1); __putc(10); }", "ws", "ws");
  expectSuccess(result, "compile ws");
  assert.equal(typeof result.output, "string", "compile ws output type");
  assert.ok(result.output.length > 0, "compile ws output length");

  const result2 = compile("func: __main() { __puti(1); __putc(10); }", "mnemonic", "ws");
  expectSuccess(result2, "compile mnemonic");
  assert.ok(result2.output.includes("push"), "compile mnemonic output");

  const result3 = compile("func: __main() {}", "invalid", "ws");
  expectFailure(result3, "compile invalid target");

  const result4 = compile("func: __main() {}", "ws", "standard");
  expectFailure(result4, "compile std mismatch");
}

// 3. parse() tests
{
  const result = parse("func: __main() { let: x; x = 1; }");
  expectSuccess(result, "parse ok");

  const result2 = parse("func: __main() { let x; }");
  expectFailure(result2, "parse error");
}

// 4. WasmWhitespaceVM tests
{
  const vm = new WasmWhitespaceVM("func: __main() { __puti(1); __putc(10); }", "");
  assert.equal(vm.is_complete(), false, "vm initial is_complete");

  const stepResult = vm.step(1000);
  assert.ok(
    ["suspended", "complete", "error"].includes(stepResult.status),
    "vm step status"
  );

  runToComplete(vm, 100, 1000);
  assert.equal(vm.is_complete(), true, "vm completion");

  const stdout = vm.flush_stdout();
  assert.equal(stdout, "1\n", "vm stdout");

  const stack = vm.get_stack();
  assert.ok(Array.isArray(stack), "vm stack type");
  const heap = vm.get_heap();
  assert.equal(typeof heap, "object", "vm heap type");
  const traced = vm.get_traced();
  assert.equal(typeof traced, "object", "vm traced type");

  const instructions = vm.disassemble();
  assert.ok(Array.isArray(instructions), "vm disassemble type");

  const compiled = compile("func: __main() { __puti(2); __putc(10); }", "ws", "ws");
  expectSuccess(compiled, "compile for fromWhitespace");
  const vm2 = WasmWhitespaceVM.fromWhitespace(compiled.output, "");
  runToComplete(vm2, 100, 1000);
  assert.equal(vm2.flush_stdout(), "2\n", "vm2 stdout");
}

// 6. Compile error tests (semantic errors)
{
  // Undefined variable (use WasmNospaceVM instead of run())
  try {
    new WasmNospaceVM("func: __main() { __puti(undefined_var); }", "");
    assert.fail("should throw on undefined variable");
  } catch (e) {
    assert.ok(true, "compile error: undefined variable");
  }

  // Undefined function
  try {
    new WasmNospaceVM("func: __main() { undefined_func(); }", "");
    assert.fail("should throw on undefined function");
  } catch (e) {
    assert.ok(true, "compile error: undefined function");
  }

  // Duplicate variable definition in same scope
  try {
    new WasmNospaceVM("func: __main() { let: x; let: x; }", "");
    assert.fail("should throw on duplicate variable");
  } catch (e) {
    assert.ok(true, "compile error: duplicate variable");
  }

  // Missing main function
  const result9 = compile("func: foo() { __puti(1); }", "ws", "ws");
  expectFailure(result9, "compile error: missing main function");

  // Assignment to undefined variable
  try {
    new WasmNospaceVM("func: __main() { undefined_var = 42; }", "");
    assert.fail("should throw on assignment to undefined variable");
  } catch (e) {
    assert.ok(true, "compile error: assignment to undefined variable");
  }

  // Parse errors
  const result7 = parse("return: 42;");
  expectFailure(result7, "parse error: return at top level");

  const result8 = parse("func: () {}");
  expectFailure(result8, "parse error: empty function name");

  const result12 = parse("func: __main() { let: arr[]; }");
  expectFailure(result12, "parse error: array without size");
}

console.log("WASM Node.js tests passed.");
