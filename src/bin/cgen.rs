// SPDX-License-Identifier: MIT

//! Debug helper: prints the direct AST-to-C output for a Rune source file.

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    let source = fs::read_to_string(&args[1]).unwrap();
    let source_file = rune::shared::source::SourceFile::new(&args[1], source);
    let tokens = rune::language::lexer::Lexer::new(&source_file)
        .tokenize()
        .unwrap();
    let program = rune::language::parser::Parser::new(tokens).parse().unwrap();
    let program = rune::language::analyze::Analyzer::new()
        .analyze(program)
        .unwrap();
    print!("{}", rune::emit::c::CGenerator::new().generate(&program));
}
