//! Transformation passes over the IR.

pub mod const_fold;
pub mod dead_code;

pub use const_fold::ConstFold;
pub use dead_code::DeadCodeElim;
