// SPDX-License-Identifier: MIT

//! Rune compiler CLI.

use rune::Compiler;
use std::env;
use std::fs;
use std::process::{self, Command};

/// Prints the command-line help text.
fn print_usage() {
    println!(
        "Rune compiler\n\
         \n\
         Usage: rune [OPTION] <file.rune>\n\
         \n\
         Options:\n\
         \x20 --wat       print the WebAssembly text format instead of building\n\
         \x20 --llvm      write output.ll and build it with clang\n\
         \x20 -h, --help  show this help\n\
         \n\
         With no option, the file is compiled to C, built with gcc, and the\n\
         executable is written to ./output."
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return;
    }

    // `rune --wat <file.rune>` prints the WebAssembly text format and exits.
    // `rune --llvm <file.rune>` writes output.ll, compiles it with clang,
    // and produces ./output.
    let (backend, path) = match args.as_slice() {
        [path] => (None, path.as_str()),
        [backend, path] if backend == "--wat" || backend == "--llvm" => {
            (Some(backend.as_str()), path.as_str())
        }
        _ => {
            eprintln!("Usage: rune [--wat | --llvm] <file.rune>");
            process::exit(1);
        }
    };
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {}: {}", path, e);
            process::exit(1);
        }
    };

    if let Some(backend) = backend {
        // Same front end and passes as the C pipeline; only the emitter
        // differs.
        let lowered = match Compiler::lower(path, &source) {
            Ok(module) => module,
            Err(msg) => {
                eprintln!("{msg}");
                process::exit(1);
            }
        };

        if backend == "--wat" {
            match rune::emit::wasm::generate(&lowered) {
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

        // --llvm: write output.ll and compile it with clang against the C
        // runtime.
        if let Err(e) = fs::write("output.ll", rune::emit::llvm::generate(&lowered)) {
            eprintln!("Failed to write output.ll: {}", e);
            process::exit(1);
        }
        let status = Command::new("clang")
            .args([
                "output.ll",
                "runtime/rune_rt.c",
                "-o",
                "output",
                "-I.",
                // the module uses the host-default triple
                "-Wno-override-module",
            ])
            .status();
        match status {
            Ok(s) if s.success() => {
                println!("Compiled successfully → ./output");
            }
            Ok(_) => {
                eprintln!("clang failed");
                process::exit(1);
            }
            Err(e) => {
                eprintln!("Failed to run clang: {}", e);
                process::exit(1);
            }
        }
        return;
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
