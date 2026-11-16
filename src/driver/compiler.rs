// SPDX-License-Identifier: MIT

//! High-level compilation pipeline.

use crate::diagnostics::reporter::Reporter;
use crate::emit::c::ir;
use crate::language::analyze::Analyzer;
use crate::language::lexer::Lexer;
use crate::language::parser::Parser;
use crate::shared::source::SourceFile;
use crate::transform::ir::Module;
use crate::transform::{ConstFold, DeadCodeElim, lower_program};

pub struct Compiler;

impl Compiler {
    /// Runs the whole front end — lex, parse, analyze — then lowers to the
    /// IR and applies the transform passes. Any stage's diagnostics are
    /// reported to stderr and `compilation failed` is returned.
    ///
    /// `require_main` enforces the executable entry point (the C and LLVM
    /// backends produce a runnable binary); `--wat` modules may omit it.
    fn build_module(
        source_name: &str,
        source_text: &str,
        require_main: bool,
    ) -> Result<Module, String> {
        let source = SourceFile::new(source_name, source_text);
        let reporter = Reporter::new(&source);

        // 1. Lex
        let tokens = match Lexer::new(&source).tokenize() {
            Ok(t) => t,
            Err(e) => {
                reporter.report(&e);
                return Err("compilation failed".to_string());
            }
        };

        // 2. Parse
        let program = match Parser::new(tokens).parse() {
            Ok(p) => p,
            Err(e) => {
                reporter.report(&e);
                return Err("compilation failed".to_string());
            }
        };

        // 3. Analyze
        let program = match Analyzer::new().require_main(require_main).analyze(program) {
            Ok(p) => p,
            Err(errors) => {
                reporter.report_all(&errors);
                return Err("compilation failed".to_string());
            }
        };

        // 4. Lower to IR and run the transform passes
        let mut module = lower_program(&program);
        ConstFold::run(&mut module);
        DeadCodeElim::run(&mut module);
        Ok(module)
    }

    /// Lowers a module for a backend that produces a runnable binary (`main`
    /// is required).
    pub fn lower_executable(source_name: &str, source_text: &str) -> Result<Module, String> {
        Self::build_module(source_name, source_text, true)
    }

    /// Lowers a module without requiring an entry point (the WASM backend).
    pub fn lower(source_name: &str, source_text: &str) -> Result<Module, String> {
        Self::build_module(source_name, source_text, false)
    }

    /// Compiles to C (see `lower_executable` for the shared front end).
    pub fn compile(source_name: &str, source_text: &str) -> Result<String, String> {
        let module = Self::lower_executable(source_name, source_text)?;
        Ok(ir::generate(&module))
    }
}
