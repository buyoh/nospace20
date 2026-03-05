mod common;

#[path = "code_test/error_base.rs"]
mod error_base;
#[path = "code_test/interpreter_base.rs"]
mod interpreter_base;
#[path = "code_test/nospace_vm_base.rs"]
mod nospace_vm_base;
#[path = "code_test/test_config.rs"]
mod test_config;
#[path = "code_test/whitespace_base.rs"]
mod whitespace_base;
#[path = "code_test/whitespace_self_base.rs"]
mod whitespace_self_base;

use error_base::*;
use interpreter_base::*;
use nospace_vm_base::*;
use whitespace_base::*;
use whitespace_self_base::*;

// ========================================
// 自動生成されたテスト
// ========================================
// テストは resources/tests/test-manifest.yaml で定義され、
// build.rs によって自動生成されます。
// テストを追加する場合は test-manifest.yaml を編集してください。

include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
