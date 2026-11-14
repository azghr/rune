//! Diagnostic and error reporting system.

pub mod diagnostic;
pub mod reporter;

pub use diagnostic::{Diagnostic, Severity};
pub use reporter::Reporter;
