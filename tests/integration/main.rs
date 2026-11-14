// SPDX-License-Identifier: MIT

//! End-to-end tests: compile Rune source to C, build it with `gcc`, run the
//! binary, and check its output. Requires `gcc` on the `PATH`.

use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use rune::driver::Compiler;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Compiles Rune source all the way to a native binary, returning its stdout.
fn run_program(source: &str) -> String {
    let c_code = Compiler::compile("test.rune", source).expect("compilation should succeed");

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let c_path = std::env::temp_dir().join(format!("rune_it_{id}.c"));
    let bin_path = std::env::temp_dir().join(format!("rune_it_{id}"));
    fs::write(&c_path, &c_code).expect("write generated C");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let status = Command::new("gcc")
        .arg(&c_path)
        .arg(format!("{manifest_dir}/runtime/rune_rt.c"))
        .arg("-o")
        .arg(&bin_path)
        .arg("-I")
        .arg(manifest_dir)
        .status()
        .expect("gcc should be available");
    assert!(status.success(), "gcc should compile the generated C");

    let output = Command::new(&bin_path).output().expect("run the binary");
    assert!(output.status.success(), "the binary should exit cleanly");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let _ = fs::remove_file(&c_path);
    let _ = fs::remove_file(&bin_path);
    stdout
}

#[test]
fn hello_world_prints_and_exits_cleanly() {
    let out = run_program(
        "fn main() {
            let x = 10
            let y = 20
            let sum = x + y * 2
            print(sum)

            if sum > 40 {
                print(1)
            } else {
                print(0)
            }
        }",
    );
    assert_eq!(out, "50\n1\n");
}

#[test]
fn while_loops_and_recursion_work_end_to_end() {
    let out = run_program(
        "fn fib(n: int) -> int {
            if n < 2 {
                return n
            }
            return fib(n - 1) + fib(n - 2)
        }

        fn main() {
            let i = 0
            while i < 10 {
                print(fib(i))
                i = i + 1
            }
        }",
    );
    assert_eq!(out, "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n");
}

#[test]
fn arithmetic_and_comparison_operators_evaluate_correctly() {
    let out = run_program(
        "fn main() {
            print(7 / 2)
            print(7 % 2)
            print(2 * 3 + 4 * 5)
            print((1 + 2) * 3)
            if 1 < 2 && 3 <= 3 {
                print(1)
            }
            if 2 > 3 || 2 == 2 {
                print(2)
            }
            if 1 != 2 {
                print(3)
            }
        }",
    );
    assert_eq!(out, "3\n1\n26\n9\n1\n2\n3\n");
}

#[test]
fn unary_operators_evaluate_correctly() {
    let out = run_program(
        "fn main() {
            let x = -5
            print(x + 10)
            if !false {
                print(1)
            }
        }",
    );
    assert_eq!(out, "5\n1\n");
}

#[test]
fn lex_errors_stop_the_pipeline() {
    let err = Compiler::compile("test.rune", "fn main() { let x = @ }")
        .expect_err("unexpected character should fail");
    assert!(err.contains("compilation failed"));
}

#[test]
fn parse_errors_stop_the_pipeline() {
    let err = Compiler::compile("test.rune", "fn main() { let x 1 }")
        .expect_err("missing `=` should fail");
    assert!(err.contains("compilation failed"));
}

#[test]
fn analysis_errors_stop_the_pipeline() {
    let err = Compiler::compile("test.rune", "fn main() { let x: bool = 1 }")
        .expect_err("type mismatch should fail");
    assert!(err.contains("compilation failed"));
}
