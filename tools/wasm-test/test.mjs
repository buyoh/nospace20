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
} = bg;

function expectSuccess(result, label) {
  assert.equal(result.success, true, `${label}: expected success`);
}

function expectFailure(result, label) {
  assert.equal(result.success, false, `${label}: expected failure`);
  assert.ok(Array.isArray(result.errors), `${label}: expected errors array`);
  assert.ok(result.errors.length > 0, `${label}: expected errors`);
}

// WasmWhitespaceVM のコンストラクタが ResultErr を投げることを確かめる
function expectConstructorThrowsResultErr(source, label) {
  let thrown;
  try {
    new WasmWhitespaceVM(source, "");
  } catch (e) {
    thrown = e;
  }
  assert.ok(thrown !== undefined, `${label}: expected the constructor to throw`);
  expectFailure(thrown, label);
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

// 1. WasmWhitespaceVM from nospace source (cases migrated from the removed WasmNospaceVM)
{
  // Basic execution
  const vm1 = new WasmWhitespaceVM("func: __main() { __puti(42); __putc(10); }", "");
  runToComplete(vm1, 1000, 1000);
  assert.equal(vm1.is_complete(), true, "vm1 completion");
  assert.equal(vm1.flush_stdout(), "42\n", "vm1 stdout");

  // stdin support
  const vm2 = new WasmWhitespaceVM(
    "func: __main() { let: x; x = __geti(); __puti(x); __putc(10); }",
    "123\n"
  );
  runToComplete(vm2, 1000, 1000);
  assert.equal(vm2.flush_stdout(), "123\n", "vm2 stdin stdout");

  // trace support (requires the debug extension)
  const vm3 = new WasmWhitespaceVM(
    "func: __main() { let: x; x = 10; __trace(x); }",
    "",
    false,
    ["debug"]
  );
  runToComplete(vm3, 1000, 1000);
  const traced = vm3.get_traced();
  assert.equal(typeof traced, "object", "vm3 traced type");
  assert.equal(traced["10"], 1, "vm3 traced value");

  // step-by-step execution (suspension)
  const vm4 = new WasmWhitespaceVM("func: __main() { __puti(1); __putc(10); }", "");
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
  assert.equal(vm4.flush_stdout(), "1\n", "vm4 stdout");

  // total_steps (number of executed Whitespace instructions)
  assert.ok(vm4.total_steps() > 0, "vm4 total_steps > 0");

  // ["debug", "alloc"] is the former WasmNospaceVM default: __assert is checked
  const vm5 = new WasmWhitespaceVM(
    "func: __main() { let: p; p = __alloc(2); __assert(0); __free(p); }",
    "",
    false,
    ["debug", "alloc"]
  );
  const vm5Result = vm5.step(100000);
  assert.equal(vm5Result.status, "error", "vm5 debug+alloc assert status");
  assert.equal(vm5Result.error, "assertion failed: 0", "vm5 debug+alloc assert error");

  // ["alloc"] is the former ignore_debug: __assert is ignored
  const vm6 = new WasmWhitespaceVM(
    "func: __main() { __assert(0); __puti(1); __putc(10); }",
    "",
    false,
    ["alloc"]
  );
  runToComplete(vm6, 1000, 1000);
  assert.equal(vm6.flush_stdout(), "1\n", "vm6 ignore_debug stdout");

  // opt_passes option
  const vm7 = new WasmWhitespaceVM(
    "func: __main() { __puti(1 + 2); __putc(10); }",
    "",
    false,
    undefined,
    ["constant-folding"]
  );
  runToComplete(vm7, 1000, 1000);
  assert.equal(vm7.flush_stdout(), "3\n", "vm7 opt_passes stdout");

  // syntax error
  expectConstructorThrowsResultErr("func: __main() { let x; }", "vm syntax error");
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

// 5. WasmWhitespaceVM options (std_extensions, opt_passes, max_memory_cells) and error strings
{
  // Runs until complete or error and returns the last step result
  function runUntilStop(vm, budget = 10000, maxIterations = 10000) {
    for (let i = 0; i < maxIterations; i += 1) {
      const result = vm.step(budget);
      if (result.status !== "suspended") {
        return result;
      }
    }
    throw new Error("vm did not stop within iteration limit");
  }

  // debug extension is applied to the VM: __trace is recorded
  const vmTrace = new WasmWhitespaceVM("func: __main() { __trace(10); }", "", false, ["debug"]);
  assert.equal(runUntilStop(vmTrace).status, "complete", "debug trace status");
  assert.equal(vmTrace.get_traced()["10"], 1, "debug trace recorded");

  // debug extension is applied to the VM: __assert(0) fails
  const vmAssert = new WasmWhitespaceVM("func: __main() { __assert(0); }", "", false, ["debug"]);
  const assertResult = runUntilStop(vmAssert);
  assert.equal(assertResult.status, "error", "debug assert status");
  assert.equal(assertResult.error, "assertion failed: 0", "debug assert error");

  // without the debug extension, __trace is not recorded
  const vmNoDebug = new WasmWhitespaceVM("func: __main() { __trace(10); }", "");
  assert.equal(runUntilStop(vmNoDebug).status, "complete", "no debug status");
  assert.equal(vmNoDebug.get_traced()["10"], undefined, "no debug trace");

  // __clog output is discarded (not mixed into stdout)
  const vmClog = new WasmWhitespaceVM(
    "func: __main() { __puti(__clog(7)); }",
    "",
    false,
    ["debug"]
  );
  assert.equal(runUntilStop(vmClog).status, "complete", "clog status");
  assert.equal(vmClog.flush_stdout(), "7", "clog stdout");

  // opt_passes gives the same output
  const optSource =
    "func: __main() { let: i; i = 0; while: i < 3 { __puti(i * 2 + 1); i = i + 1; }; }";
  const vmNoOpt = new WasmWhitespaceVM(optSource, "");
  runUntilStop(vmNoOpt);
  const vmOpt = new WasmWhitespaceVM(optSource, "", false, undefined, ["all"]);
  assert.equal(runUntilStop(vmOpt).status, "complete", "opt_passes status");
  assert.equal(vmOpt.flush_stdout(), vmNoOpt.flush_stdout(), "opt_passes stdout");

  // runtime error string uses the display message
  const vmDiv = new WasmWhitespaceVM(
    "func: __main() { let: x; x = __geti(); __puti(1 / x); }",
    "0\n"
  );
  const divResult = runUntilStop(vmDiv);
  assert.equal(divResult.status, "error", "division by zero status");
  assert.equal(divResult.error, "division by zero", "division by zero error");

  // a small max_memory_cells stops non-tail infinite recursion
  const recursionSource = "func: f() { return: f() + 1; } func: __main() { __puti(f()); }";
  const vmLimit = new WasmWhitespaceVM(recursionSource, "", false, undefined, undefined, 1000);
  const limitResult = runUntilStop(vmLimit);
  assert.equal(limitResult.status, "error", "memory limit status");
  assert.equal(limitResult.error, "memory limit exceeded: 1000 cells", "memory limit error");

  // the default limit also stops it
  const vmDefaultLimit = new WasmWhitespaceVM(recursionSource, "");
  const defaultLimitResult = runUntilStop(vmDefaultLimit, 1000000, 10000);
  assert.equal(defaultLimitResult.status, "error", "default memory limit status");
  assert.match(defaultLimitResult.error, /^memory limit exceeded: \d+ cells$/, "default memory limit error");

  // max_memory_cells = 0 is unlimited
  const vmUnlimited = new WasmWhitespaceVM(recursionSource, "", false, undefined, undefined, 0);
  assert.equal(vmUnlimited.step(100000).status, "suspended", "unlimited status");
}

// 6. Compile error tests (semantic errors)
{
  // Undefined variable
  expectConstructorThrowsResultErr("func: __main() { __puti(undefined_var); }", "compile error: undefined variable");

  // Undefined function
  expectConstructorThrowsResultErr("func: __main() { undefined_func(); }", "compile error: undefined function");

  // Duplicate variable definition in same scope
  expectConstructorThrowsResultErr("func: __main() { let: x; let: x; }", "compile error: duplicate variable");

  // Missing main function
  const result9 = compile("func: foo() { __puti(1); }", "ws", "ws");
  expectFailure(result9, "compile error: missing main function");

  // Assignment to undefined variable
  expectConstructorThrowsResultErr("func: __main() { undefined_var = 42; }", "compile error: assignment to undefined variable");

  // Parse errors
  const result7 = parse("return: 42;");
  expectFailure(result7, "parse error: return at top level");

  const result8 = parse("func: () {}");
  expectFailure(result8, "parse error: empty function name");

  const result12 = parse("func: __main() { let: arr[]; }");
  expectFailure(result12, "parse error: array without size");
}

console.log("WASM Node.js tests passed.");
