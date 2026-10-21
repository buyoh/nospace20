//! WebAssembly public API
//!
//! Exposes functionality equivalent to CLI for JavaScript.
//! Only compiled when `wasm` feature is enabled.
//!
//! ## Module Structure
//!
//! - [`types`]: TypeScript type definitions and Serde result structures
//! - [`pipeline`]: Common compilation pipeline, parameter parsers, and error conversion
//! - [`api`]: Top-level API (`compile`, `parse`) and helpers
//! - [`whitespace_vm`]: WASM wrapper for Whitespace VM

mod api;
mod pipeline;
mod types;
mod whitespace_vm;
