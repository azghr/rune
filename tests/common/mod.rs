// SPDX-License-Identifier: MIT

//! Shared helpers for the test targets: thin wrappers that run each
//! pipeline stage and panic with a useful message if it fails.
//!
//! Each test target uses a different subset of these helpers.

#![allow(dead_code)]

use rune::driver::Compiler;
use rune::emit::c::CGenerator;
use rune::language::analyze::Analyzer;
use rune::language::ast::Program;
use rune::language::lexer::Lexer;
use rune::language::parser::Parser;
use rune::language::token::Token;
use rune::shared::source::SourceFile;

/// Builds a `SourceFile` for the given text, as if it came from `test.rune`.
pub fn source(text: &str) -> SourceFile {
    SourceFile::new("test.rune", text)
}

/// Lexes `text`, panicking on lexer errors.
pub fn tokens(text: &str) -> Vec<Token> {
    Lexer::new(&source(text))
        .tokenize()
        .expect("lexing should succeed")
}

/// Runs the full front end (lex, parse, analyze) on `text`, panicking on any
/// stage's errors.
pub fn analyze(text: &str) -> Program {
    Analyzer::new()
        .analyze(parse(text))
        .expect("analysis should succeed")
}

/// Parses `text`, panicking on parser errors.
pub fn parse(text: &str) -> Program {
    Parser::new(tokens(text))
        .parse()
        .expect("parsing should succeed")
}

/// Generates C for `text` by running the whole pipeline.
pub fn generate(text: &str) -> String {
    CGenerator::new().generate(&analyze(text))
}

/// Runs the analyzer expecting errors, returning the collected diagnostics.
pub fn analyze_errors(text: &str) -> Vec<rune::diagnostics::Diagnostic> {
    Analyzer::new()
        .analyze(parse(text))
        .expect_err("analysis should fail")
}

/// Parses `text` expecting an error, returning the diagnostic.
pub fn parse_error(text: &str) -> rune::diagnostics::Diagnostic {
    Parser::new(tokens(text))
        .parse()
        .expect_err("parsing should fail")
}

/// Runs the full `Compiler::compile` entry point.
pub fn compile(text: &str) -> Result<String, String> {
    Compiler::compile("test.rune", text)
}
