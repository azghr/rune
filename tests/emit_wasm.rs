// SPDX-License-Identifier: MIT

//! Tests for the WebAssembly (text format) backend.
//!
//! Structural checks run everywhere; the final test validates the generated
//! WAT with `wat2wasm` (wabt) when it is on the PATH. CI installs wabt and
//! additionally runs the generated module in Node.

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
fn function_values_are_rejected_with_a_clear_error() {
    let module = common::analyze(
        "fn twice(n: int) -> int { return n * 2 }
        fn main() { let f = twice print(f(3)) }",
    );
    let err =
        wasm::generate(&lower_program(&module)).expect_err("function values should be rejected");
    assert!(err.contains("does not support"));
    assert!(err.contains("`main`"));
}

/// Validates the generated WAT with wabt's `wat2wasm` when installed.
/// CI installs wabt, so this runs there; locally it skips gracefully.
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
