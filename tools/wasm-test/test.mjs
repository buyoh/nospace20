import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const wasm = require("../../pkg/nospace20.js");

const {
  run,
  compile,
  parse,
  compile_to_whitespace_string,
  compile_to_mnemonic_string,
  WasmWhitespaceVM,
} = wasm;

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

// 1. run() tests
{
  const result = run("func main() { __println(42); }", "", false);
  expectSuccess(result, "run basic");
  assert.equal(result.stdout, "42\n", "run stdout");

  const result2 = run(
    "func main() { let: x; __readInt(x); __println(x); }",
    "123\n",
    false
  );
  expectSuccess(result2, "run stdin");
  assert.equal(result2.stdout, "123\n", "run stdin stdout");

  const result3 = run("func main() { let: x; x = 10; __trace(x); }", "", true);
  expectSuccess(result3, "run debug");
  assert.ok(result3.trace !== undefined, "run debug trace");

  const result4 = run("func main() { let x; }", "", false);
  expectFailure(result4, "run syntax error");
}

// 2. compile() tests
{
  const result = compile("func main() { __println(1); }", "ws", "ws");
  expectSuccess(result, "compile ws");
  assert.equal(typeof result.output, "string", "compile ws output type");
  assert.ok(result.output.length > 0, "compile ws output length");

  const result2 = compile("func main() { __println(1); }", "mnemonic", "ws");
  expectSuccess(result2, "compile mnemonic");
  assert.ok(result2.output.includes("push"), "compile mnemonic output");

  const result3 = compile("func main() {}", "invalid", "ws");
  expectFailure(result3, "compile invalid target");

  const result4 = compile("func main() {}", "ws", "standard");
  expectFailure(result4, "compile std mismatch");
}

// 3. parse() tests
{
  const result = parse("func main() { let: x; x = 1; }");
  expectSuccess(result, "parse ok");

  const result2 = parse("func main() { let x; }");
  expectFailure(result2, "parse error");
}

// 4. helper compile tests
{
  const ws = compile_to_whitespace_string("func main() { __println(3); }");
  expectSuccess(ws, "compile_to_whitespace_string");
  assert.ok(ws.output.length > 0, "compile_to_whitespace_string output");

  const mnemonic = compile_to_mnemonic_string("func main() { __println(3); }");
  expectSuccess(mnemonic, "compile_to_mnemonic_string");
  assert.ok(mnemonic.output.includes("push"), "compile_to_mnemonic_string output");
}

// 5. WasmWhitespaceVM tests
{
  const vm = new WasmWhitespaceVM("func main() { __println(1); }", "");
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

  const compiled = compile("func main() { __println(2); }", "ws", "ws");
  expectSuccess(compiled, "compile for fromWhitespace");
  const vm2 = WasmWhitespaceVM.fromWhitespace(compiled.output, "");
  runToComplete(vm2, 100, 1000);
  assert.equal(vm2.flush_stdout(), "2\n", "vm2 stdout");
}

console.log("WASM Node.js tests passed.");
