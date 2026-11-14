// SPDX-License-Identifier: MIT

//! Debug helper: prints the IR blocks for a Rune source file.

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.iter().find(|a| a.ends_with(".rune")).unwrap();
    let source = fs::read_to_string(path).unwrap();
    let source_file = rune::shared::source::SourceFile::new(path.as_str(), source);
    let tokens = rune::language::lexer::Lexer::new(&source_file)
        .tokenize()
        .unwrap();
    let program = rune::language::parser::Parser::new(tokens).parse().unwrap();
    let program = rune::language::analyze::Analyzer::new()
        .analyze(program)
        .unwrap();
    let mut module = rune::transform::lower_program(&program);
    if env::args().any(|a| a == "--passes" || a == "--fold") {
        rune::transform::ConstFold::run(&mut module);
    }
    if env::args().any(|a| a == "--passes" || a == "--dce") {
        rune::transform::DeadCodeElim::run(&mut module);
    }
    for func in &module.functions {
        println!("fn {} ({:?}):", func.name, func.return_type);
        for block in &func.body {
            println!("  L{}: {:?}", block.id, block.instructions);
            println!("      -> {:?}", block.terminator);
        }
    }
}
