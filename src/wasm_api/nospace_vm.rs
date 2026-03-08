//! WASM wrapper for NospaceVM
//!
//! Construct a VM capable of step execution from nospace source.
//! Provides the same interface pattern as `WasmWhitespaceVM`.
//!
//! Used as an alternative to the `run()` API (recursive interpreter).
//! For one-shot execution, implement with `step()` loop:
//! ```javascript
//! const vm = new WasmNospaceVM(source, stdin);
//! while (true) {
//!   const result = vm.step(100000);
//!   if (result.status !== 'suspended') break;
//! }
//! const stdout = vm.flushStdout();
//! ```

use std::cell::RefCell;
use std::io::{BufReader, Cursor};
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::base::shared_writer::SharedWriter;
use crate::interpreter::vm::{NospaceVM, StepResult};
use crate::EnvironmentConfig;

use super::pipeline;
use super::types::{JsOptPassArray, JsVmStepResult, ResultErr, VmStepResult};

// ========================================
// WasmNospaceVM
// ========================================

/// WASM wrapper for NospaceVM
///
/// Treated as an opaque type on the JS side; manipulate state via method calls.
/// Provides the same interface pattern as `WasmWhitespaceVM`.
#[wasm_bindgen]
pub struct WasmNospaceVM {
    vm: NospaceVM,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}

#[wasm_bindgen]
impl WasmNospaceVM {
    /// Construct VM from nospace source code
    ///
    /// - `stdin`: Contents of standard input
    /// - `opt_passes`: Array of optimization passes (optional; e.g., `["all"]`)
    /// - `ignore_debug`: Whether to ignore debug built-in functions (optional, defaults to false)
    #[wasm_bindgen(constructor)]
    pub fn new(
        source: &str,
        stdin: &str,
        opt_passes: Option<JsOptPassArray>,
        ignore_debug: Option<bool>,
    ) -> Result<WasmNospaceVM, JsValue> {
        // Parse + optimize
        let (scope, _text_code, _) = pipeline::analyze_and_optimize(source, opt_passes)
            .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        // Build I/O buffers
        let stdin_cursor: Box<dyn std::io::BufRead> =
            Box::new(BufReader::new(Cursor::new(stdin.as_bytes().to_vec())));
        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_clone = Rc::clone(&stdout_buf);
        let stdout_writer: Box<dyn std::io::Write> = Box::new(SharedWriter(stdout_clone));

        // Build VM
        let mut vm = NospaceVM::from_scope(scope).map_err(|e| {
            let err = ResultErr::single_error(format!("{}", e));
            serde_wasm_bindgen::to_value(&err).unwrap()
        })?;

        // I/O configuration (with_io disables stdout_capture, so directly manipulate fields)
        vm = vm.with_io(stdin_cursor, stdout_writer);

        // ignore_debug configuration
        if ignore_debug.unwrap_or(false) {
            let mut config = EnvironmentConfig::default();
            config.ignore_debug = true;
            vm = vm.with_config(config);
        }

        Ok(WasmNospaceVM {
            vm,
            stdout_buffer: stdout_buf,
        })
    }

    /// Execute specified number of steps
    ///
    /// Returns: VmStepResult ({ status: "suspended" | "complete" | "error", error?: string })
    pub fn step(&mut self, budget: u32) -> JsVmStepResult {
        let result = self.vm.step(budget as usize);

        let vm_result = match result {
            StepResult::Suspended => VmStepResult {
                status: "suspended".to_string(),
                error: None,
                input_type: None,
            },
            StepResult::Complete { .. } => VmStepResult {
                status: "complete".to_string(),
                error: None,
                input_type: None,
            },
            StepResult::Error(e) => VmStepResult {
                status: "error".to_string(),
                error: Some(format!("{}", e)),
                input_type: None,
            },
        };

        let js: JsValue = serde_wasm_bindgen::to_value(&vm_result).unwrap();
        js.into()
    }

    /// Whether execution is complete
    pub fn is_complete(&self) -> bool {
        self.vm.is_complete()
    }

    /// Total number of expression evaluations
    pub fn total_steps(&self) -> usize {
        self.vm.total_steps()
    }

    /// Get and clear stdout buffer contents
    #[wasm_bindgen(js_name = "flushStdout")]
    pub fn flush_stdout(&mut self) -> String {
        self.vm.flush();
        let mut buf = self.stdout_buffer.borrow_mut();
        let text = String::from_utf8_lossy(&buf).to_string();
        buf.clear();
        text
    }

    /// Get return value (only valid when complete)
    #[wasm_bindgen(js_name = "getReturnValue")]
    pub fn get_return_value(&self) -> Option<i64> {
        self.vm.return_value()
    }

    /// Get trace information
    ///
    /// Returns: { [key: string]: number }
    #[wasm_bindgen(js_name = "getTraced")]
    pub fn get_traced(&self) -> JsValue {
        let obj = js_sys::Object::new();
        for (k, v) in self.vm.traced().iter() {
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str(&k.to_string()),
                &JsValue::from_f64(*v as f64),
            )
            .unwrap();
        }
        obj.into()
    }
}
