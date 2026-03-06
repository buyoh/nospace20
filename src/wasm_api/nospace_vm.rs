//! NospaceVM の WASM ラッパー
//!
//! nospace ソースからステップ実行可能な VM を構築する。
//! `WasmWhitespaceVM` と同パターンのインターフェースを提供する。
//!
//! `run()` API（再帰インタプリタ）の代替として使用する。
//! ワンショット実行が必要な場合は `step()` ループで実現可能:
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

/// NospaceVM の WASM ラッパー
///
/// JS 側ではオペーク型として扱われ、メソッド呼び出しで状態を操作する。
/// `WasmWhitespaceVM` と同パターンのインターフェースを提供する。
#[wasm_bindgen]
pub struct WasmNospaceVM {
    vm: NospaceVM,
    stdout_buffer: Rc<RefCell<Vec<u8>>>,
}

#[wasm_bindgen]
impl WasmNospaceVM {
    /// nospace ソースコードから VM を構築する
    ///
    /// - `stdin`: 標準入力の内容
    /// - `opt_passes`: 最適化パスの配列（省略可; 例: `["all"]`）
    /// - `ignore_debug`: デバッグ用組み込み関数を無視するか（省略可、デフォルト false）
    #[wasm_bindgen(constructor)]
    pub fn new(
        source: &str,
        stdin: &str,
        opt_passes: Option<JsOptPassArray>,
        ignore_debug: Option<bool>,
    ) -> Result<WasmNospaceVM, JsValue> {
        // 解析 + 最適化
        let (scope, _text_code, _) = pipeline::analyze_and_optimize(source, opt_passes)
            .map_err(|e| serde_wasm_bindgen::to_value(&e).unwrap())?;

        // I/O バッファ構築
        let stdin_cursor: Box<dyn std::io::BufRead> =
            Box::new(BufReader::new(Cursor::new(stdin.as_bytes().to_vec())));
        let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
        let stdout_clone = Rc::clone(&stdout_buf);
        let stdout_writer: Box<dyn std::io::Write> = Box::new(SharedWriter(stdout_clone));

        // VM 構築
        let mut vm = NospaceVM::from_scope(scope).map_err(|e| {
            let err = ResultErr::single_error(format!("{}", e));
            serde_wasm_bindgen::to_value(&err).unwrap()
        })?;

        // I/O 設定 (with_io は stdout_capture を無効化するため、直接フィールドを操作)
        vm = vm.with_io(stdin_cursor, stdout_writer);

        // ignore_debug 設定
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

    /// 指定ステップ数だけ実行する
    ///
    /// 戻り値: VmStepResult ({ status: "suspended" | "complete" | "error", error?: string })
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

    /// 実行完了済みか
    pub fn is_complete(&self) -> bool {
        self.vm.is_complete()
    }

    /// 総式評価回数
    pub fn total_steps(&self) -> usize {
        self.vm.total_steps()
    }

    /// 標準出力バッファの内容を取得しクリアする
    #[wasm_bindgen(js_name = "flushStdout")]
    pub fn flush_stdout(&mut self) -> String {
        self.vm.flush();
        let mut buf = self.stdout_buffer.borrow_mut();
        let text = String::from_utf8_lossy(&buf).to_string();
        buf.clear();
        text
    }

    /// 戻り値を取得（完了時のみ有効）
    #[wasm_bindgen(js_name = "getReturnValue")]
    pub fn get_return_value(&self) -> Option<i64> {
        self.vm.return_value()
    }

    /// トレース情報を取得
    ///
    /// 戻り値: { [key: string]: number }
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
