// SPDX-License-Identifier: MIT

//! Code generation backends.

pub mod c;
pub mod llvm;
pub mod wasm;

pub use c::CGenerator;
