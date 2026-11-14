// SPDX-License-Identifier: MIT

//! Source location tracking.
//!
//! A `Span` represents a half-open byte range in the original source file.
//! It is attached to almost every AST node and token so that diagnostics
//! can point to the exact location of problems.

#![allow(dead_code)]

use std::ops::Range;

/// A half-open byte range `[start, end)` in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }

    pub fn to_range(self) -> Range<usize> {
        self.start..self.end
    }

    /// Merge two spans into the smallest span that covers both.
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
