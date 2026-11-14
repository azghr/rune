//! High-level compilation pipeline.

use crate::diagnostics::reporter::Reporter;
use crate::emit::c::CGenerator;
use crate::language::analyze::Analyzer;
use crate::language::lexer::Lexer;
use crate::language::parser::Parser;
use crate::shared::source::SourceFile;

pub struct Compiler;

impl Compiler {
    pub fn compile(source_name: &str, source_text: &str) -> Result<String, ()> {
        let source = SourceFile::new(source_name, source_text);
        let reporter = Reporter::new(&source);

        // 1. Lex
        let tokens = match Lexer::new(&source).tokenize() {
            Ok(t) => t,
            Err(e) => {
                reporter.report(&e);
                return Err(());
            }
        };

        // 2. Parse
        let program = match Parser::new(tokens).parse() {
            Ok(p) => p,
            Err(e) => {
                reporter.report(&e);
                return Err(());
            }
        };

        // 3. Analyze
        let program = match Analyzer::new().analyze(program) {
            Ok(p) => p,
            Err(errors) => {
                reporter.report_all(&errors);
                return Err(());
            }
        };

        // 4. Emit C
        let c_code = CGenerator::new().generate(&program);
        Ok(c_code)
    }
}
