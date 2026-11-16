// SPDX-License-Identifier: MIT

//! Rune compiler CLI.

use rune::Compiler;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};

/// Locates the runtime include root — the directory holding
/// `runtime/rune_rt.c` and `runtime/rune_rt.h`.
///
/// If `$RUNE_RUNTIME_DIR` is set it must point at such a directory; a bad
/// value is an error rather than a silent fallback. Otherwise the working
/// directory and then the compiler executable's directory are each searched
/// upward (the latter covers `target/release/rune` built by cargo from the
/// repository root).
fn find_runtime_root() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("RUNE_RUNTIME_DIR") {
        let root = PathBuf::from(&dir);
        if root.join("runtime").join("rune_rt.c").is_file() {
            return Ok(root);
        }
        return Err(format!(
            "RUNE_RUNTIME_DIR (`{dir}`) does not contain `runtime/rune_rt.c`"
        ));
    }

    let mut starts = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = env::current_exe()
        && let Some(parent) = exe.parent()
    {
        starts.push(parent.to_path_buf());
    }

    for start in starts {
        for dir in start.ancestors() {
            if dir.join("runtime").join("rune_rt.c").is_file() {
                return Ok(dir.to_path_buf());
            }
        }
    }
    Err(
        "Failed to locate the Rune runtime (runtime/rune_rt.c). Set \
         RUNE_RUNTIME_DIR to the directory containing the `runtime` folder, \
         or run from inside the repository."
            .to_string(),
    )
}

/// Resolves the runtime include root and its `rune_rt.c` source, exiting
/// with a diagnostic if they cannot be found. Only the C and LLVM backends
/// need this — `--wat` is self-contained.
fn runtime_paths() -> (PathBuf, PathBuf) {
    match find_runtime_root() {
        Ok(root) => {
            let runtime_c = root.join("runtime").join("rune_rt.c");
            (root, runtime_c)
        }
        Err(msg) => {
            eprintln!("{msg}");
            process::exit(1);
        }
    }
}

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
        // Same front end and passes for all backends; only the emitter
        // differs. WAT needs no `main` (the module is a library) and no C
        // runtime.
        if backend == "--wat" {
            let lowered = match Compiler::lower(path, &source) {
                Ok(module) => module,
                Err(msg) => {
                    eprintln!("{msg}");
                    process::exit(1);
                }
            };
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

        let lowered = match Compiler::lower_executable(path, &source) {
            Ok(module) => module,
            Err(msg) => {
                eprintln!("{msg}");
                process::exit(1);
            }
        };

        // --llvm: write output.ll and compile it with clang against the C
        // runtime.
        let (runtime_root, runtime_c) = runtime_paths();
        if let Err(e) = fs::write("output.ll", rune::emit::llvm::generate(&lowered)) {
            eprintln!("Failed to write output.ll: {}", e);
            process::exit(1);
        }
        let status = Command::new("clang")
            .arg("output.ll")
            .arg(&runtime_c)
            .arg("-o")
            .arg("output")
            .arg("-I")
            .arg(&runtime_root)
            // the module uses the host-default triple
            .arg("-Wno-override-module")
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

            // Compile with gcc against the C runtime.
            let (runtime_root, runtime_c) = runtime_paths();
            let status = Command::new("gcc")
                .arg("output.c")
                .arg(&runtime_c)
                .arg("-o")
                .arg("output")
                .arg("-I")
                .arg(&runtime_root)
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
