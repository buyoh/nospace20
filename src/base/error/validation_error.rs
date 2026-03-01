//! バリデーションエラー型
//!
//! コンパイルプロパティのバリデーションエラーを表す。
//!
//! NOTE: ValidationError の定義は `compile_property` で行われており、
//! `LanguageStd`/`CompileTarget` との循環依存を避けるため、ここでは re-export のみ行う。

pub use crate::compile_property::ValidationError;
