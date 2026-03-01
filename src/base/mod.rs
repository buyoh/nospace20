pub mod error;
pub mod shared_writer;
pub mod ws_types;

// 後方互換のため CodeParseError を base モジュール直下で公開
pub use error::CodeParseError;

mod location;
pub use location::SourceLocation;

pub mod pure_eval;
pub mod constexpr_eval;
