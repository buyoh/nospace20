//! WASM wrapper for Whitespace VM
//!
//! Construct a VM capable of step execution from nospace or Whitespace source.

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::runner::{build_vm, WsRunOptions};
use crate::whitespace::{
    max_memory_cells_from_setting, InputWaitType, StepResult, WhitespaceVM,
    DEFAULT_MAX_MEMORY_CELLS_WASM,
};
use crate::WsOutputFormat;

use super::pipeline;
use super::types::{
    JsNumberArray, JsNumberRecord, JsOptPassArray, JsStdExtensionArray, JsStringArray,
    JsVmStepResult, ResultErr, VmStepResult,
};

// ========================================
// SharedWriter
// ========================================

pub(super) use crate::base::shared_writer::SharedWriter;

// ========================================
// Whitespace VM helpers
// ========================================

/// Build the VM options from the VM-related arguments of the constructors
///
/// Shared by `new`, `fromWhitespace` and `fromWhitespaceInteractive` so that the same arguments
/// mean the same thing. `max_memory_cells`: `None` uses `DEFAULT_MAX_MEMORY_CELLS_WASM`, `0` is unlimited.
fn vm_options_from_args(
    debug_ext: bool,
    max_memory_cells: Option<u32>,
    strict_heap: Option<bool>,
    randomize_heap: Option<bool>,
) -> WsRunOptions {
    WsRunOptions {
        debug_ext,
        max_memory_cells: match max_memory_cells {
            Some(cells) => max_memory_cells_from_setting(cells as usize),
            None => Some(DEFAULT_MAX_MEMORY_CELLS_WASM),
        },
        strict_heap: strict_heap.unwrap_or(false),
        randomize_heap: randomize_heap.unwrap_or(false),
    }
}

/// Parse `std_extensions` for the VM: returns whether the debug extension is enabled
///
/// `alloc` is a compile-time extension and has no counterpart in the VM. It is accepted and ignored
/// so that the same array can be passed to `compile()`, `new` and `fromWhitespace*`
/// (unlike `whitespace20 --std-ext alloc`, which is an error).
fn debug_ext_from_std_extensions(
    std_extensions: Option<JsStdExtensionArray>,
) -> Result<bool, JsValue> {
    let (debug_ext, _alloc_ext) = pipeline::parse_std_extensions(std_extensions)
        .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;
    Ok(debug_ext)
}

/// Common helper to construct VM from Whitespace source
///
/// Shared by `new`, `from_whitespace` and `from_whitespace_interactive`.
/// Applies `with_interactive_stdin()` when `interactive=true`.
fn create_from_ws_source(
    ws_source: &str,
    initial_stdin: &str,
    interactive: bool,
    options: &WsRunOptions,
) -> Result<WasmWhitespaceVM, JsValue> {
    // バグ修正: 従来は with_debug_ext(false) を固定で呼んでいたため、`new()` に `["debug"]` を渡しても
    // コンパイラだけが debug 拡張付きのコードを出力し、VM は __trace を記録せず __assert も失敗しなかった。
    // 解決策: 呼び出し側から VM の設定（debug_ext を含む）を受け取って VM に反映する
    // （fromWhitespace* も std_extensions で debug 拡張を指定できる）。
    // __clog の出力先（clog writer）は設定しない（出力は捨てる）。
    let vm = build_vm(ws_source, options).map_err(|e| {
        let err = ResultErr::single_error(format!("Whitespace parse error: {:?}", e));
        serde_wasm_bindgen::to_value(&err).unwrap()
    })?;
    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let stdout_clone = Rc::clone(&stdout_buf);

    if interactive {
        let vm = vm.with_interactive_stdin();
        let mut vm_with_io = vm.with_stdout(Box::new(SharedWriter(stdout_clone)));
        if !initial_stdin.is_empty() {
            vm_with_io.provide_stdin(initial_stdin);
        }
        Ok(WasmWhitespaceVM {
            vm: vm_with_io,
            stdout_buffer: stdout_buf,
        })
    } else {
        let stdin_buf = Box::new(std::io::BufReader::new(Cursor::new(
            initial_stdin.as_bytes().to_vec(),
        )));
        let vm_with_io = vm.with_io(stdin_buf, Box::new(SharedWriter(stdout_clone)));
        Ok(WasmWhitespaceVM {
            vm: vm_with_io,
            stdout_buffer: stdout_buf,
        })
    }
}

// ========================================
// WasmWhitespaceVM
// ========================================

/// WASM wrapper for Whitespace VM
///
/// Treated as an opaque type on the JS side; manipulate state via method calls.
#[wasm_bindgen]
pub struct WasmWhitespaceVM {
    vm: WhitespaceVM,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}

#[wasm_bindgen]
impl WasmWhitespaceVM {
    /// Compile nospace source and construct Whitespace VM
    ///
    /// - `std_extensions`: Array of extensions to enable (e.g., `["debug", "alloc"]`)
    /// - `opt_passes`: Array of optimization passes (e.g., `["all"]`)
    /// - `max_memory_cells`: Memory limit of the VM in cells (heap addresses + data stack + return addresses).
    ///   `undefined` / `null` use `DEFAULT_MAX_MEMORY_CELLS_WASM` (4,194,304); only `0` means unlimited.
    ///   The value is converted to u32 by wasm-bindgen (e.g. a negative number wraps), so pass a non-negative integer
    /// - `strict_heap`: Reading an uninitialized heap address is a runtime error (default: false)
    /// - `randomize_heap`: Reading an uninitialized heap address returns a deterministic non-zero value
    ///   (default: false; `strict_heap` takes precedence when both are true)
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(constructor)]
    pub fn new(
        nospace_source: &str,
        stdin: &str,
        interactive: Option<bool>,
        std_extensions: Option<JsStdExtensionArray>,
        opt_passes: Option<JsOptPassArray>,
        max_memory_cells: Option<u32>,
        strict_heap: Option<bool>,
        randomize_heap: Option<bool>,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        let (debug_ext, alloc_ext) = pipeline::parse_std_extensions(std_extensions)
            .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        let ws_source = pipeline::compile_nospace(
            nospace_source,
            opt_passes,
            debug_ext,
            alloc_ext,
            WsOutputFormat::Whitespace,
        )
        .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        let options =
            vm_options_from_args(debug_ext, max_memory_cells, strict_heap, randomize_heap);
        create_from_ws_source(&ws_source, stdin, interactive.unwrap_or(false), &options)
    }

    /// Construct VM directly from Whitespace source code
    ///
    /// The VM arguments are the same as the tail of `new`:
    /// - `std_extensions`: `["debug"]` enables the debug extension of the VM.
    ///   `"alloc"` is a compile-time extension and has no effect here (accepted so the same array as for `new` can be passed)
    /// - `max_memory_cells`: `undefined` / `null` use `DEFAULT_MAX_MEMORY_CELLS_WASM` (4,194,304); only `0` means unlimited
    /// - `strict_heap` / `randomize_heap`: same as `new`
    #[wasm_bindgen(js_name = "fromWhitespace")]
    pub fn from_whitespace(
        ws_source: &str,
        stdin: &str,
        std_extensions: Option<JsStdExtensionArray>,
        max_memory_cells: Option<u32>,
        strict_heap: Option<bool>,
        randomize_heap: Option<bool>,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        let debug_ext = debug_ext_from_std_extensions(std_extensions)?;
        let options =
            vm_options_from_args(debug_ext, max_memory_cells, strict_heap, randomize_heap);
        create_from_ws_source(ws_source, stdin, false, &options)
    }

    /// Construct VM from Whitespace source in interactive mode
    ///
    /// When stdin buffer is empty, suspends with WaitingForInput.
    /// Can add data later with provide_stdin().
    /// The VM arguments are the same as `fromWhitespace`.
    #[wasm_bindgen(js_name = "fromWhitespaceInteractive")]
    pub fn from_whitespace_interactive(
        ws_source: &str,
        initial_stdin: &str,
        std_extensions: Option<JsStdExtensionArray>,
        max_memory_cells: Option<u32>,
        strict_heap: Option<bool>,
        randomize_heap: Option<bool>,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        let debug_ext = debug_ext_from_std_extensions(std_extensions)?;
        let options =
            vm_options_from_args(debug_ext, max_memory_cells, strict_heap, randomize_heap);
        create_from_ws_source(ws_source, initial_stdin, true, &options)
    }

    /// Add data to stdin (for interactive mode)
    ///
    /// Call when in WaitingForInput state to retry input on next step().
    /// For InputNumber, must provide with newline (\n).
    #[wasm_bindgen(js_name = "provideStdin")]
    pub fn provide_stdin(&mut self, data: &str) {
        self.vm.provide_stdin(data);
    }

    /// Notify end of stdin stream (for interactive mode)
    ///
    /// After this, if an input instruction is reached with an empty buffer, it will be treated as EOF.
    #[wasm_bindgen(js_name = "closeStdin")]
    pub fn close_stdin(&mut self) {
        self.vm.close_stdin();
    }

    /// Execute specified number of steps
    ///
    /// Returns: { status: "suspended" | "complete" | "error" | "waiting_for_input", error?: string, inputType?: string }
    pub fn step(&mut self, budget: u32) -> JsVmStepResult {
        let result = self.vm.step(budget as usize);

        let vm_result = match result {
            StepResult::Suspended => VmStepResult {
                status: "suspended".to_string(),
                error: None,
                input_type: None,
            },
            StepResult::Complete => VmStepResult {
                status: "complete".to_string(),
                error: None,
                input_type: None,
            },
            StepResult::Error(e) => VmStepResult {
                status: "error".to_string(),
                error: Some(e.to_string()),
                input_type: None,
            },
            StepResult::WaitingForInput(input_type) => VmStepResult {
                status: "waiting_for_input".to_string(),
                error: None,
                input_type: Some(match input_type {
                    InputWaitType::Char => "char".to_string(),
                    InputWaitType::Number => "number".to_string(),
                }),
            },
        };

        let js: JsValue = serde_wasm_bindgen::to_value(&vm_result).unwrap();
        js.into()
    }

    /// Current program counter (instruction index)
    pub fn pc(&self) -> usize {
        self.vm.pc()
    }

    /// Total number of instructions executed
    pub fn total_steps(&self) -> usize {
        self.vm.total_steps()
    }

    /// Whether execution is complete
    pub fn is_complete(&self) -> bool {
        self.vm.is_complete()
    }

    /// Current contents of data stack
    ///
    /// Returns: number[] (i64 → JS number conversion. Precision drops for values > 53 bits)
    pub fn get_stack(&self) -> JsNumberArray {
        let stack: Vec<f64> = self.vm.data_stack().iter().map(|&v| v as f64).collect();
        let js: JsValue = serde_wasm_bindgen::to_value(&stack).unwrap();
        js.into()
    }

    /// Current contents of heap
    ///
    /// Returns: { [address: string]: number }
    pub fn get_heap(&self) -> JsNumberRecord {
        let heap: std::collections::BTreeMap<String, f64> = self
            .vm
            .heap()
            .iter()
            .map(|(k, v)| (k.to_string(), *v as f64))
            .collect();
        let js: JsValue = serde_wasm_bindgen::to_value(&heap).unwrap();
        js.into()
    }

    /// Depth of call stack
    pub fn call_stack_depth(&self) -> usize {
        self.vm.call_stack_depth()
    }

    /// Get and clear stdout buffer contents
    pub fn flush_stdout(&mut self) -> String {
        let mut buf = self.stdout_buffer.borrow_mut();
        let text = String::from_utf8_lossy(&buf).to_string();
        buf.clear();
        text
    }

    /// Get trace information
    ///
    /// Returns: { [key: string]: number }
    pub fn get_traced(&self) -> JsValue {
        let obj = js_sys::Object::new();
        for (k, v) in self.vm.traced.iter() {
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str(&k.to_string()),
                &JsValue::from_f64(*v as f64),
            )
            .unwrap();
        }
        obj.into()
    }

    /// Get mnemonic representation of current instruction (for debugging)
    pub fn current_instruction(&self) -> Option<String> {
        self.vm.current_instruction()
    }

    /// Get mnemonic representation of entire instruction sequence
    pub fn disassemble(&self) -> JsStringArray {
        let instructions = self.vm.disassemble();
        let js: JsValue = serde_wasm_bindgen::to_value(&instructions).unwrap();
        js.into()
    }
}

#[cfg(test)]
#[path = "whitespace_vm_tests.rs"]
mod tests;
