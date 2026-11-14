// SPDX-License-Identifier: MIT

//! Semantic analysis (type checking + name resolution).

pub mod checker;
pub mod scope;
pub mod types;

pub use checker::Analyzer;
