//! WASM wrapper for Whitespace VM
//!
//! Construct a VM capable of step execution from nospace or Whitespace source.

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::whitespace::{InputWaitType, StepResult, WhitespaceVM};
use crate::{compile_to_ws, WsCompileOptions};

use super::pipeline;
use super::types::{
    JsNumberArray, JsNumberRecord, JsStdExtensionArray, JsStringArray, JsVmStepResult, ResultErr,
    VmStepResult,
};

// ========================================
// SharedWriter
// ========================================

pub(super) use crate::base::shared_writer::SharedWriter;

// ========================================
// Whitespace VM helpers
// ========================================

/// Common helper to construct VM from Whitespace source
///
/// Eliminates duplication between `from_whitespace` and `from_whitespace_interactive`.
/// Applies `with_interactive_stdin()` when `interactive=true`.
fn create_from_ws_source(
    ws_source: &str,
    initial_stdin: &str,
    interactive: bool,
) -> Result<WasmWhitespaceVM, JsValue> {
    let vm = WhitespaceVM::from_source(ws_source).map_err(|e| {
        let err = ResultErr::single_error(format!("Whitespace parse error: {:?}", e));
        serde_wasm_bindgen::to_value(&err).unwrap()
    })?;

    let vm = vm.with_debug_ext(false);
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
    #[wasm_bindgen(constructor)]
    pub fn new(
        nospace_source: &str,
        stdin: &str,
        interactive: Option<bool>,
        std_extensions: Option<JsStdExtensionArray>,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        let (debug_ext, alloc_ext) = pipeline::parse_std_extensions(std_extensions)
            .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        let (scope, text_code) = pipeline::analyze_source(nospace_source)
            .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        // Compile
        let ws_options = WsCompileOptions {
            debug_ext,
            alloc_ext,
            ..Default::default()
        };
        let ws_source = compile_to_ws(&scope, &ws_options).map_err(|e| {
            serde_wasm_bindgen::to_value(&pipeline::convert_compile_error(&e, &text_code)).unwrap()
        })?;

        create_from_ws_source(&ws_source, stdin, interactive.unwrap_or(false))
    }

    /// Construct VM directly from Whitespace source code
    #[wasm_bindgen(js_name = "fromWhitespace")]
    pub fn from_whitespace(ws_source: &str, stdin: &str) -> Result<WasmWhitespaceVM, JsValue> {
        create_from_ws_source(ws_source, stdin, false)
    }

    /// Construct VM from Whitespace source in interactive mode
    ///
    /// When stdin buffer is empty, suspends with WaitingForInput.
    /// Can add data later with provide_stdin().
    #[wasm_bindgen(js_name = "fromWhitespaceInteractive")]
    pub fn from_whitespace_interactive(
        ws_source: &str,
        initial_stdin: &str,
    ) -> Result<WasmWhitespaceVM, JsValue> {
        create_from_ws_source(ws_source, initial_stdin, true)
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
                error: Some(format!("{:?}", e)),
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
