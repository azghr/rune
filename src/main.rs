// SPDX-License-Identifier: MIT

//! Rune compiler CLI.

use rune::Compiler;
use std::env;
use std::fs;
use std::process::{self, Command};

fn main() {
    let args: Vec<String> = env::args().collect();

    // `rune --wat <file.rune>` prints the WebAssembly text format and exits.
    let emit_wat = args.len() == 3 && args[1] == "--wat";
    if !(args.len() == 2 || emit_wat) {
        eprintln!("Usage: rune [--wat] <file.rune>");
        process::exit(1);
    }

    let path = &args[args.len() - 1];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {}: {}", path, e);
            process::exit(1);
        }
    };

    if emit_wat {
        use rune::emit::wasm;
        use rune::language::analyze::Analyzer;
        use rune::language::lexer::Lexer;
        use rune::language::parser::Parser;
        use rune::shared::source::SourceFile;
        use rune::transform::lower_program;

        let source_file = SourceFile::new(path, &source);
        let wat = Lexer::new(&source_file)
            .tokenize()
            .and_then(|tokens| Parser::new(tokens).parse())
            .map_err(|e| {
                rune::diagnostics::Reporter::new(&source_file).report(&e);
                "compilation failed".to_string()
            })
            .and_then(|program| {
                Analyzer::new().analyze(program).map_err(|errors| {
                    rune::diagnostics::Reporter::new(&source_file).report_all(&errors);
                    "compilation failed".to_string()
                })
            })
            .map(|program| lower_program(&program))
            .and_then(|module| wasm::generate(&module));
        match wat {
            Ok(wat) => {
                println!("{wat}");
                return;
            }
            Err(msg) => {
                eprintln!("{msg}");
                process::exit(1);
            }
        }
    }

    match Compiler::compile(path, &source) {
        Ok(c_code) => {
            // Write generated C
            if let Err(e) = fs::write("output.c", &c_code) {
                eprintln!("Failed to write output.c: {}", e);
                process::exit(1);
            }

            // Compile with gcc
            let status = Command::new("gcc")
                .args(["output.c", "runtime/rune_rt.c", "-o", "output", "-I."])
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("Compiled successfully → ./output");
                }
                Ok(_) => {
                    eprintln!("gcc failed");
                    process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to run gcc: {}", e);
                    process::exit(1);
                }
            }
        }
        Err(msg) => {
            eprintln!("{msg}");
            process::exit(1);
        }
    }
}
