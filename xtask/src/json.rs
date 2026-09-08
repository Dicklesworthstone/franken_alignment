//! Operator inputs use the reviewed syntax parser without sharing admission
//! decisions or adding a package dependency. Both source files are bound by the
//! reviewed source manifest; canonical draft validation is not imported here.

#[path = "../../crates/fa-reference/src/strict_json.rs"]
mod syntax;

pub use syntax::*;
