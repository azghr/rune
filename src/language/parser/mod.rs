// SPDX-License-Identifier: MIT

//! Recursive-descent parser for Rune.
//!
//! Entry point: `parse_program`.

mod decl;
mod expr;
mod stmt;

use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::ast::Program;
use crate::language::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(mut self) -> Result<Program, Diagnostic> {
        let mut functions = Vec::new();
        while !self.is_at_end() {
            functions.push(self.parse_function()?);
        }
        Ok(Program { functions })
    }

    // ----- token helpers
    pub(crate) fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    pub(crate) fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    pub(crate) fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    pub(crate) fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    pub(crate) fn matches(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    pub(crate) fn consume(&mut self, kind: TokenKind, msg: &str) -> Result<&Token, Diagnostic> {
        if self.check(&kind) {
            Ok(self.advance())
        } else {
            Err(Diagnostic::error(msg, self.peek().span))
        }
    }
}
