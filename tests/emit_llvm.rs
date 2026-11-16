// SPDX-License-Identifier: MIT

//! Tests for the LLVM (textual IR) backend.
//!
//! Structural checks run everywhere; the final test compiles and executes
//! the generated `.ll` with `clang` when it is on the PATH, skipping
//! gracefully when it is not installed.

mod common;

use rune::emit::llvm;
use rune::transform::lower_program;
use rune::transform::{ConstFold, DeadCodeElim};

fn ll(text: &str) -> String {
    let module = common::analyze(text);
    let mut module = lower_program(&module);
    ConstFold::run(&mut module);
    DeadCodeElim::run(&mut module);
    llvm::generate(&module)
}

#[test]
fn module_has_runtime_decls_and_a_main_entry_point() {
    let out = ll("fn main() { print(1) }");
    assert!(out.contains("declare void @rune_print_int(i64)"));
    assert!(out.contains("declare ptr @rune_closure_new(ptr, i32)"));
    assert!(out.contains("define void @f_main() {"));
    assert!(out.contains("define i32 @main() {\n  call void @f_main()\n  ret i32 0\n}"));
}

#[test]
fn ints_map_to_i64_and_bools_to_i1() {
    let out = ll("fn main() { let x = 7 let b = x > 3 print(x) print(b) }");
    assert!(out.contains("%l_x = alloca i64"));
    assert!(out.contains("icmp sgt i64"));
    assert!(out.contains("%l_b = alloca i1"));
    assert!(out.contains("zext i1"));
    assert!(out.contains("call void @rune_print_int(i64"));
    assert!(out.contains("call void @rune_print_bool(i8"));
}

#[test]
fn the_cfg_becomes_labeled_blocks() {
    let out = ll(include_str!("../examples/fib.rune"));
    assert!(out.contains("br label %L"));
    assert!(out.contains("br i1"));
    assert!(out.contains("ret i64"));
    assert!(out.contains("define i64 @f_fib(i64 %arg_n) {"));
}

#[test]
fn string_literals_become_globals_and_use_the_runtime() {
    let out = ll("fn main() { print(\"hi\") print(\"a\" + \"b\") print(\"a\" == \"b\") }");
    assert!(out.contains("@.str.0 = private unnamed_addr constant [3 x i8] c\"hi\\00\""));
    assert!(out.contains("getelementptr inbounds [3 x i8], ptr @.str.0"));
    assert!(out.contains("call ptr @rune_strcat(ptr"));
    assert!(out.contains("call i32 @strcmp(ptr"));
    assert!(out.contains("call void @rune_print_string(ptr"));
}

#[test]
fn function_references_become_closures_and_indirect_calls() {
    let out = ll("fn twice(n: int) -> int { return n * 2 }
        fn main() { let f = twice print(f(3)) }");
    // the closure block is allocated via the C runtime's helpers
    assert!(out.contains("call ptr @rune_closure_new(ptr @f_rune_closure_twice, i32 0)"));
    // the env-first wrapper forwards to the real function, dropping the env
    assert!(out.contains(
        "define i64 @f_rune_closure_twice(ptr %env, i64 %arg_n) {\n  %ret = call i64 @f_twice(i64 %arg_n)"
    ));
    // the call loads the closure, then its code word, then calls through it
    // with the closure as the environment argument
    assert!(out.contains("= load ptr, ptr %l_f"));
    assert!(out.contains("= load ptr, ptr %v"));
    assert!(out.contains("= call i64 %v"));
    assert!(out.contains("(ptr %v"));
    assert!(out.contains(", i64 3)"));
}

#[test]
fn captured_variables_are_stored_in_the_closure_env() {
    let out = ll(
        "fn make_adder(n: int) -> fn(int) -> int { return fn(x: int) -> int { return x + n } }
        fn main() { let add1 = make_adder(1) print(add1(10)) }",
    );
    // the lambda is hoisted with the capture appended as a parameter
    assert!(out.contains("define i64 @f_rune_lambda_0(i64 %arg_x, i64 %arg_n) {"));
    // the capture is stored into the closure block's i64 slot
    assert!(out.contains("call void @rune_closure_set(ptr %v0, i32 0, i64"));
    // the wrapper loads it from the env and forwards it
    assert!(out.contains("%cap0 = getelementptr i64, ptr %env, i64 1"));
    assert!(out.contains("%raw0 = load i64, ptr %cap0"));
    assert!(out.contains("call i64 @f_rune_lambda_0(i64 %arg_x, i64 %raw0)"));
}

#[test]
fn narrow_captures_are_widened_and_narrowed() {
    let out =
        ll("fn main() { let b = true let f = fn() -> bool { return b } if f() { print(1) } }");
    // bool captures are zext on store and trunc on load
    assert!(out.contains("zext i1"));
    assert!(out.contains("trunc i64 %raw0 to i1"));
}

#[test]
fn void_signatures_work_end_to_end() {
    let out = ll("fn main() { let f = fn(x: int) { print(x) } f(1) }");
    assert!(out.contains("define void @f_rune_lambda_0(i64 %arg_x) {"));
    // a void indirect call has no result register to name
    assert!(out.contains("call void %"));
}

/// Compiles and executes the generated `.ll` with clang when installed.
#[test]
fn generated_llvm_compiles_and_runs() {
    let Ok(_) = std::process::Command::new("clang")
        .arg("--version")
        .output()
    else {
        eprintln!("skipping: clang is not installed");
        return;
    };

    let cases = [
        ("fib", include_str!("../examples/fib.rune"), "0\n1\n1\n2\n3\n5\n8\n13\n21\n34"),
        (
            "capturing_lambda",
            "fn make_adder(n: int) -> fn(int) -> int { return fn(x: int) -> int { return x + n } }
            fn main() { let add1 = make_adder(1) let add2 = make_adder(2) print(add1(10)) print(add2(10)) }",
            "11\n12",
        ),
        (
            "fn_typed_param",
            "fn apply(f: fn(int) -> int, x: int) -> int { return f(x) }
            fn inc(n: int) -> int { return n + 1 }
            fn main() { print(apply(inc, 5)) }",
            "6",
        ),
    ];
    let dir = std::env::temp_dir().join(format!("rune_llvm_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    for (name, source, expected) in cases {
        let module = common::analyze(source);
        let mut module = lower_program(&module);
        ConstFold::run(&mut module);
        DeadCodeElim::run(&mut module);
        let text = llvm::generate(&module);
        let ll_path = dir.join(format!("{name}.ll"));
        let bin_path = dir.join(name);
        std::fs::write(&ll_path, &text).expect("write .ll");
        let runtime = std::env::current_dir().unwrap().join("runtime/rune_rt.c");
        let status = std::process::Command::new("clang")
            .args([
                ll_path.to_str().unwrap(),
                runtime.to_str().unwrap(),
                "-o",
                bin_path.to_str().unwrap(),
                "-Wno-override-module",
            ])
            .output()
            .expect("run clang");
        assert!(
            status.status.success(),
            "clang rejected {name}:\n{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let run = std::process::Command::new(&bin_path)
            .output()
            .expect("run binary");
        let stdout = String::from_utf8_lossy(&run.stdout);
        assert_eq!(stdout.trim_end(), expected, "wrong output for {name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
