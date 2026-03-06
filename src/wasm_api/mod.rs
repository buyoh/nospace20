//! WebAssembly 公開 API
//!
//! CLI と同等の機能を JavaScript から呼び出し可能にする。
//! `wasm` feature が有効な場合のみコンパイルされる。
//!
//! ## モジュール構成
//!
//! - [`types`]: TypeScript 型定義・Serde 結果構造体
//! - [`pipeline`]: 共通コンパイルパイプライン・パラメータパーサ・エラー変換
//! - [`api`]: トップレベル API（`compile`, `parse`）およびヘルパー
//! - [`whitespace_vm`]: Whitespace VM の WASM ラッパー
//! - [`nospace_vm`]: NospaceVM の WASM ラッパー（`run()` API の代替）

mod api;
mod nospace_vm;
mod pipeline;
mod types;
mod whitespace_vm;
