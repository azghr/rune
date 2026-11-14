// SPDX-License-Identifier: MIT

//! Rune compiler library.

pub mod diagnostics;
pub mod driver;
pub mod emit;
pub mod language;
pub mod shared;
pub mod transform; // still mostly empty for now

pub use driver::Compiler;
