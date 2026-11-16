// SPDX-License-Identifier: MIT

//! Source file abstraction.
//!
//! Holds the raw source text and provides helpers for turning byte offsets
//! into human-readable line/column information.

use crate::shared::span::Span;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub content: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let mut line_starts = vec![0];
        for (i, b) in content.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            name: name.into(),
            content,
            line_starts,
        }
    }

    pub fn span_to_line_col(&self, span: Span) -> (usize, usize) {
        let line = match self.line_starts.binary_search(&span.start) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        // Columns count characters, not bytes, so the caret under the
        // source line lines up even with multibyte characters before the
        // span.
        let col = self.content[self.line_starts[line]..span.start]
            .chars()
            .count();
        (line + 1, col + 1) // 1-based
    }

    pub fn slice(&self, span: Span) -> &str {
        &self.content[span.to_range()]
    }
}
