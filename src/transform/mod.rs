// SPDX-License-Identifier: MIT

//! Intermediate representation and transformation passes.
//!
//! Currently provides a simple IR and two example passes:
//! constant folding and dead code elimination.

pub mod ir;
pub mod lower;
pub mod passes;

pub use ir::*;
pub use lower::lower_program;
pub use passes::*;
