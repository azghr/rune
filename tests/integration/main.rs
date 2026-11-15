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
            let mut i = 0
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
fn else_if_chains_evaluate_correctly() {
    let out = run_program(
        "fn classify(n: int) -> int {
            if n < 0 {
                return -1
            } else if n == 0 {
                return 0
            } else {
                return 1
            }
        }

        fn main() {
            print(classify(-5))
            print(classify(0))
            print(classify(7))
        }",
    );
    assert_eq!(out, "-1\n0\n1\n");
}

#[test]
fn string_concatenation_works_end_to_end() {
    let out = run_program(
        "fn main() {
            let greeting = \"hello, \"
            let name = \"rune\"
            print(greeting + name)
            print(\"a\" + \"b\" + \"c\")
            if greeting + name == \"hello, rune\" {
                print(1)
            }
            if \"ab\" != \"a\" + \"b\" {
                print(2)
            }
        }",
    );
    assert_eq!(out, "hello, rune\nabc\n1\n");
}

#[test]
fn string_comparison_compares_contents() {
    let out = run_program(
        "fn main() {
            let a = \"hello\"
            let b = \"world\"
            print(a == \"hello\")
            print(a == b)
            print(a != b)
            if \"x\" != \"y\" {
                print(1)
            }
        }",
    );
    assert_eq!(out, "true\nfalse\ntrue\n1\n");
}

#[test]
fn print_dispatches_bool_and_string_arguments() {
    let out = run_program(
        "fn main() {
            let flag = true
            let name = \"rune\"
            print(flag)
            print(false)
            print(name)
            print(42)
        }",
    );
    assert_eq!(out, "true\nfalse\nrune\n42\n");
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
fn compound_assignment_works_end_to_end() {
    let out = run_program(
        "fn main() {
            let mut x = 1
            x += 2
            x *= 3
            x -= 1
            print(x)
            let mut s = \"a\"
            s += \"b\"
            s += \"c\"
            print(s)
        }",
    );
    assert_eq!(out, "8\nabc\n");
}

#[test]
fn for_loops_work_end_to_end() {
    let out = run_program(
        "fn main() {
            let mut sum = 0
            for let mut i = 0; i < 5; i += 1 {
                sum += i
            }
            print(sum)

            let mut pow = 1
            for ; pow < 100; pow *= 2 {
                print(pow)
            }
        }",
    );
    assert_eq!(out, "10\n1\n2\n4\n8\n16\n32\n64\n");
}

#[test]
fn break_and_continue_work_in_while_loops() {
    let out = run_program(
        "fn main() {
            let mut i = 0
            let mut hits = 0
            while i < 10 {
                i += 1
                if i == 3 {
                    continue
                }
                if i > 6 {
                    break
                }
                hits += 1
            }
            print(hits)
        }",
    );
    // skips 3, breaks after 6: counts 1, 2, 4, 5, 6
    assert_eq!(out, "5\n");
}

#[test]
fn for_loop_continue_still_runs_the_step() {
    // if `continue` skipped the step clause, this loop would never terminate
    let out = run_program(
        "fn main() {
            for let mut n = 0; n < 5; n += 1 {
                if n == 2 {
                    continue
                }
                if n == 4 {
                    break
                }
                print(n)
            }
        }",
    );
    assert_eq!(out, "0\n1\n3\n");
}

#[test]
fn break_in_nested_loops_only_exits_the_inner_one() {
    let out = run_program(
        "fn main() {
            let mut x = 0
            while x < 3 {
                let mut y = 0
                while y < 3 {
                    y += 1
                    if y == 2 {
                        break
                    }
                }
                x += 1
                if x == 1 {
                    continue
                }
                print(x)
            }
        }",
    );
    assert_eq!(out, "2\n3\n");
}

#[test]
fn reassigning_from_a_call_beats_constant_propagation() {
    // Regression: ConstFold once kept `x -> 1` across `x = f()` and
    // folded `print(x)` into `print(1)`.
    let out =
        run_program("fn f() -> int { return 7 } fn main() { let mut x = 1 x = f() print(x) }");
    assert_eq!(out, "7\n");
}

#[test]
fn functions_are_first_class_values_end_to_end() {
    let out = run_program(
        "fn apply(f: fn(int) -> int, x: int) -> int {
            return f(x)
        }
        fn twice(n: int) -> int {
            return n * 2
        }
        fn triple(n: int) -> int {
            return n * 3
        }
        fn pick(big: bool) -> fn(int) -> int {
            if big {
                return triple
            }
            return twice
        }
        fn main() {
            let f = twice
            print(f(5))
            print(apply(twice, 10))
            print(apply(triple, 10))
            let g = pick(true)
            print(g(4))
            let h = pick(false)
            print(h(4))
        }",
    );
    assert_eq!(out, "10\n20\n30\n12\n8\n");
}

#[test]
fn chained_calls_work_end_to_end() {
    let out = run_program(
        "fn twice(n: int) -> int {
            return n * 2
        }
        fn triple(n: int) -> int {
            return n * 3
        }
        fn pick(big: bool) -> fn(int) -> int {
            if big {
                return triple
            }
            return twice
        }
        fn main() {
            print(pick(true)(7))
            print(pick(false)(7))
            let f = pick(false)
            print(f(pick(true)(2)))
        }",
    );
    assert_eq!(out, "21\n14\n12\n");
}

#[test]
fn string_escapes_round_trip_through_generated_c() {
    // the lexer decodes escapes; the emitters must re-encode them or the
    // generated C is not even valid
    let out = run_program(
        "fn main() {
            print(\"a\\tb\")
            print(\"q\\\"q\")
            print(\"back\\\\slash\")
            let s = \"line1\\nline2\"
            print(s == \"line1\\nline2\")
        }",
    );
    assert_eq!(out, "a\tb\nq\"q\nback\\slash\ntrue\n");
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
