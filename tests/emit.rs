// SPDX-License-Identifier: MIT

//! C generation tests: signatures, declarations, type mapping, operators.

mod common;

use common::generate;

#[test]
fn emits_c_headers_and_runtime_include() {
    let c = generate("fn main() { print(1) }");
    assert!(c.contains("#include <stdint.h>"));
    assert!(c.contains("#include <stdbool.h>"));
    assert!(c.contains("#include \"runtime/rune_rt.h\""));
}

#[test]
fn emits_forward_declarations() {
    let c = generate("fn fib(n: int) -> int { return n } fn main() { print(fib(5)) }");
    assert!(
        c.contains("int64_t fib(int64_t n);"),
        "missing fib prototype in:\n{c}"
    );
    assert!(c.contains("int main();"), "missing main prototype in:\n{c}");
    // prototypes come before definitions
    assert!(
        c.find("int64_t fib(int64_t n);").unwrap() < c.find("int64_t fib(int64_t n) {").unwrap()
    );
}

#[test]
fn maps_rune_types_to_c_types() {
    let c =
        generate("fn f(i: int, b: bool, s: string) -> string { return s } fn main() { print(1) }");
    assert!(c.contains("const char* f(int64_t i, bool b, const char* s)"));
}

#[test]
fn main_is_emitted_as_c_int_main_with_return_zero() {
    let c = generate("fn main() { print(1) }");
    assert!(c.contains("int main() {"));
    assert!(c.contains("return 0;"));
}

#[test]
fn maps_print_to_the_runtime_call() {
    let c = generate("fn main() { print(1 + 2) }");
    assert!(c.contains("rune_print_int("));
}

#[test]
fn emits_typed_variable_declarations() {
    let c = generate(
        "fn main() { let i = 1 let mut b = true let s = \"hi\" print(i) print(b) print(s) }",
    );
    assert!(c.contains("int64_t i = 1;"));
    assert!(c.contains("bool b = true;"));
    assert!(c.contains("const char* s = \"hi\";"));
}

#[test]
fn dispatches_print_by_argument_type() {
    let c = generate("fn main() { print(1) print(true) print(\"hi\") }");
    assert!(c.contains("rune_print_int(1)"));
    assert!(c.contains("rune_print_bool(true)"));
    assert!(c.contains("rune_print_string(\"hi\")"));
}

#[test]
fn maps_arithmetic_operators_verbatim() {
    let c = generate("fn main() { let x = 1 + 2 - 3 * 4 / 5 % 6 print(x) }");
    for op in ["+", "-", "*", "/", "%"] {
        assert!(c.contains(op), "missing operator `{op}` in:\n{c}");
    }
}

#[test]
fn maps_logical_operators_verbatim() {
    let c = generate("fn main() { let x = true && false || true print(x) }");
    assert!(c.contains("&&"));
    assert!(c.contains("||"));
}

#[test]
fn string_equality_is_emitted_as_strcmp() {
    let c = generate("fn main() { let a = \"x\" print(a == \"y\") print(a != \"x\") }");
    assert!(c.contains("strcmp(a, \"y\") == 0"));
    assert!(c.contains("strcmp(a, \"x\") != 0"));
}

#[test]
fn string_concatenation_is_emitted_as_rune_strcat() {
    let c = generate("fn main() { let a = \"x\" print(a + \"y\") print(\"a\" + \"b\" == \"ab\") }");
    assert!(c.contains("rune_strcat(a, \"y\")"));
    assert!(c.contains("rune_print_string"));
    assert!(c.contains("strcmp("));
}

#[test]
fn emits_if_else_and_while_as_c_control_flow() {
    let c = generate(
        "fn main() {
            let mut i = 0
            while i < 3 {
                if i == 1 { print(i) } else { print(0) }
                i = i + 1
            }
        }",
    );
    assert!(c.contains("while ("));
    assert!(c.contains("if ("));
    assert!(c.contains("else"));
}

#[test]
fn generates_the_full_fib_program() {
    let c = generate(include_str!("../examples/fib.rune"));
    assert!(c.contains("int64_t fib(int64_t n)"));
    assert!(c.contains("fib((n - 1))"));
    assert!(c.contains("return 0;"));
}
