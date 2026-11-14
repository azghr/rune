//! Rune compiler CLI.

use rune::Compiler;
use std::env;
use std::fs;
use std::process::{self, Command};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rune <file.rune>");
        process::exit(1);
    }

    let path = &args[1];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {}: {}", path, e);
            process::exit(1);
        }
    };

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
        Err(()) => process::exit(1),
    }
}
