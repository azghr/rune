// SPDX-License-Identifier: MIT

//! Pretty-prints diagnostics with source context.

use crate::diagnostics::diagnostic::{Diagnostic, Severity};
use crate::shared::source::SourceFile;

pub struct Reporter<'a> {
    source: &'a SourceFile,
}

impl<'a> Reporter<'a> {
    pub fn new(source: &'a SourceFile) -> Self {
        Self { source }
    }

    pub fn report(&self, diag: &Diagnostic) {
        let (line, col) = self.source.span_to_line_col(diag.span);
        let severity = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };

        eprintln!(
            "{}:{}:{}: {}: {}",
            self.source.name, line, col, severity, diag.message
        );

        // Simple context line
        if let Some(line_text) = self.source.content.lines().nth(line - 1) {
            eprintln!("  {}", line_text);
            eprint!("  ");
            for _ in 0..col.saturating_sub(1) {
                eprint!(" ");
            }
            eprintln!("^");
        }
    }

    pub fn report_all(&self, diagnostics: &[Diagnostic]) {
        for d in diagnostics {
            self.report(d);
        }
    }
}
