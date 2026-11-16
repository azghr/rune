// SPDX-License-Identifier: MIT

//! Tests for the WebAssembly (text format) backend.
//!
//! Structural checks run everywhere; the final test validates the generated
//! WAT with `wat2wasm` (wabt) when it is on the PATH, skipping gracefully
//! when it is not installed.

mod common;

use rune::emit::wasm;
use rune::transform::lower_program;
use rune::transform::{ConstFold, DeadCodeElim};

fn wat(text: &str) -> String {
    let module = common::analyze(text);
    let mut module = lower_program(&module);
    ConstFold::run(&mut module);
    DeadCodeElim::run(&mut module);
    wasm::generate(&module).expect("the WASM backend should accept this program")
}

fn wat_unoptimized(text: &str) -> String {
    let module = common::analyze(text);
    wasm::generate(&lower_program(&module)).expect("the WASM backend should accept this program")
}

#[test]
fn module_has_imports_and_main_export() {
    let out = wat("fn main() { print(1) }");
    assert!(out.starts_with("(module"));
    assert!(out.contains("(import \"rune\" \"print_int\" (func $f_rune_print_int (param i64)))"));
    assert!(out.contains("(import \"rune\" \"print_bool\""));
    assert!(out.contains("(func $f_main"));
    assert!(out.contains("(export \"main\" (func $f_main))"));
}

#[test]
fn ints_map_to_i64_and_bools_to_i32() {
    let out =
        wat("fn main() { let x = 7 let mut b = true if x > 3 { b = false } print(x) print(b) }");
    assert!(out.contains("i64.const 7"));
    assert!(out.contains("i64.gt_s"));
    assert!(out.contains("i32.const 1"));
    assert!(out.contains("i32.eqz") || out.contains("local.set $l_b"));
    assert!(out.contains("call $f_rune_print_int"));
    assert!(out.contains("call $f_rune_print_bool"));
}

#[test]
fn the_cfg_becomes_a_dispatch_loop() {
    let out = wat(include_str!("../examples/fib.rune"));
    assert!(out.contains("(loop $rune_top"));
    assert!(out.contains("br_table"));
    // every terminator transitions through the dispatch register
    assert!(out.contains("local.set $rune_pc"));
    assert!(out.contains("br $rune_top"));
}

#[test]
fn arithmetic_operators_map_to_wasm_ops() {
    // pre-pass lowering: constant folding would collapse the expression
    let out = wat_unoptimized("fn main() { print(1 + 2 - 3 * 4 / 5 % 6) }");
    for op in ["i64.add", "i64.sub", "i64.mul", "i64.div_s", "i64.rem_s"] {
        assert!(out.contains(op), "missing {op}");
    }
}

#[test]
fn functions_become_wasm_functions() {
    let out = wat(include_str!("../examples/fib.rune"));
    assert!(out.contains("(func $f_fib (param $l_n i64) (result i64)"));
    assert!(out.contains("call $f_fib"));
}

#[test]
fn string_literals_become_data_segments_and_pointers() {
    let out = wat("fn main() { print(\"hi\") }");
    assert!(out.contains("(data (i32.const 1024) \"hi\\00\")"));
    assert!(out.contains("(i32.const 1024)"));
    assert!(out.contains("call $f_rune_print_string"));
    assert!(out.contains("(memory (export \"rune_mem\") 1)"));
}

#[test]
fn string_concat_and_comparison_use_the_runtime_prelude() {
    let out =
        wat("fn main() { let a = \"x\" print(a + \"y\") print(a == \"x\") print(a != \"x\") }");
    assert!(out.contains("call $f_rune_strcat"));
    assert!(out.contains("call $f_rune_strcmp"));
    assert!(out.contains("i32.eqz"));
    assert!(out.contains("(global $rune_heap"));
    assert!(out.contains("$f_rune_alloc"));
    assert!(out.contains("$f_rune_strlen"));
}

#[test]
fn function_references_become_table_entries_and_indirect_calls() {
    let out = wat("fn twice(n: int) -> int { return n * 2 }
        fn main() { let f = twice print(f(3)) }");
    // one table entry and one env-first wrapper per closure target
    assert!(out.contains("(table 1 funcref)"));
    assert!(out.contains("(elem (i32.const 0) func $f_rune_closure_twice)"));
    assert!(out.contains(
        "(func $f_rune_closure_twice (param $rune_env i32) (param $l_n i64) (result i64)"
    ));
    // the closure block holds the table index as its code word
    assert!(out.contains("i32.const 8"));
    assert!(out.contains("call $f_rune_alloc"));
    assert!(out.contains("i32.store"));
    // the call reads the code word and goes through the table
    assert!(out.contains("i32.load"));
    assert!(out.contains("call_indirect (type $t0)"));
    assert!(out.contains("(type $t0 (func (param i32 i64) (result i64)))"));
}

#[test]
fn anonymous_functions_hoist_and_wrap() {
    let out = wat("fn main() { let f = fn(x: int) -> int { return x + 1 } print(f(1)) }");
    assert!(out.contains("(func $f_rune_lambda_0"));
    assert!(out.contains("(elem (i32.const 0) func $f_rune_closure_rune_lambda_0)"));
    assert!(out.contains("call_indirect (type $t0)"));
}

#[test]
fn captured_variables_are_stored_in_the_closure_env() {
    let out = wat("fn main() {
            let a = 3
            let b = true
            let f = fn() -> int { if b { return a } return 0 }
            print(f())
        }");
    // captures are stored as i64 slots past the code word
    assert!(out.contains("i64.store offset=8"));
    // a bool capture is narrowed back; an int capture is forwarded raw
    assert!(out.contains("(i32.wrap_i64 (i64.load offset=8 (local.get $rune_env)))"));
    assert!(out.contains("(i64.load offset=16 (local.get $rune_env))"));
    // two captures, both slots, i64 slots widened from i32 where needed
    assert!(out.contains("i64.extend_i32_u"));
    assert!(out.contains("i64.store offset=16"));
}

#[test]
fn void_signatures_get_no_result_type_and_discarded_values_are_dropped() {
    // an if/else lambda body produces a void-returning signature
    let out = wat("fn main() { let f = fn(x: int) { if x > 0 { print(x) } } f(1) }");
    assert!(out.contains("(type $t0 (func (param i32 i64)))"));
    assert!(!out.contains("(type $t0 (func (param i32 i64)) (result"));
    // a non-void call made as a statement must drop its result
    let out = wat("fn g(n: int) -> int { return n } fn main() { g(1) print(2) }");
    assert!(out.contains("drop"));
}

#[test]
fn string_arguments_to_indirect_calls_get_data_segments() {
    let out = wat("fn main() {
            let g = fn(s: string) -> string { return s + \"!\" }
            print(g(\"hey\"))
        }");
    // the argument literal must have a data segment, not fall back to
    // address 0
    assert!(out.contains("(data (i32.const 1024) \"hey\\00\")"));
    assert!(out.contains("call_indirect (type $t0)"));
    assert!(out.contains("(type $t0 (func (param i32 i32) (result i32)))"));
}

/// Validates the generated WAT with wabt's `wat2wasm` when installed.
/// Locally it skips gracefully when wabt is absent.
#[test]
fn generated_wat_is_valid_wasm() {
    let Ok(_) = std::process::Command::new("wat2wasm")
        .arg("--version")
        .output()
    else {
        eprintln!("skipping: wat2wasm (wabt) is not installed");
        return;
    };

    let cases = [
        (
            "hello",
            "fn main() { let x = 10 let y = 20 let sum = x + y * 2 print(sum) if sum > 40 { print(1) } else { print(0) } }",
        ),
        ("fib", include_str!("../examples/fib.rune")),
        (
            "function_reference",
            "fn twice(n: int) -> int { return n * 2 } fn main() { let f = twice print(f(3)) }",
        ),
        (
            "capturing_lambda",
            "fn make_adder(n: int) -> fn(int) -> int { return fn(x: int) -> int { return x + n } }
            fn main() { let add1 = make_adder(1) let add2 = make_adder(2) print(add1(10)) print(add2(10)) }",
        ),
        (
            "fn_typed_param",
            "fn apply(f: fn(int) -> int, x: int) -> int { return f(x) }
            fn inc(n: int) -> int { return n + 1 }
            fn main() { print(apply(inc, 5)) }",
        ),
        (
            "void_lambda_statement",
            "fn g(n: int) -> int { return n } fn main() { g(1) let f = fn(x: int) { print(x) } f(2) }",
        ),
    ];
    for (name, source) in cases {
        let text = wat(source);
        let path = std::env::temp_dir().join(format!("rune_wat_{}.wat", std::process::id()));
        std::fs::write(&path, &text).expect("write WAT");
        let status = std::process::Command::new("wat2wasm")
            .arg(&path)
            .arg("-o")
            .arg(path.with_extension("wasm"))
            .output()
            .expect("run wat2wasm");
        assert!(
            status.status.success(),
            "wat2wasm rejected {name}:\n{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("wasm"));
    }
}
