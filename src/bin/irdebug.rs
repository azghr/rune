// SPDX-License-Identifier: MIT

//! Debug helper: prints the IR blocks for a Rune source file.

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
    let module = rune::transform::lower_program(&program);
    for func in &module.functions {
        println!("fn {} ({:?}):", func.name, func.return_type);
        for block in &func.body {
            println!("  L{}: {:?}", block.id, block.instructions);
            println!("      -> {:?}", block.terminator);
        }
    }
}
