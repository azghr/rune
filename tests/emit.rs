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
fn anonymous_functions_become_named_c_functions() {
    // through the real pipeline: lowering hoists the lambda, so both the
    // generated function and its pointer-typed binding appear in the C
    let c = common::compile("fn main() { let f = fn(x: int) -> int { return x + 1 } print(f(1)) }")
        .expect("compiles");
    assert!(c.contains("int64_t rune_lambda_0(int64_t x);"));
    // function values are closure pointers; the lambda is wrapped env-first
    assert!(c.contains("void* f;"));
    assert!(c.contains("rune_closure_new((void *)rune_closure_rune_lambda_0, 0);"));
    assert!(c.contains("int64_t rune_closure_rune_lambda_0(void *env, int64_t x)"));
    // calls read the code word and pass the closure as the environment
    assert!(c.contains("((int64_t (*)(void *, int64_t))(*((void **)f)))( f, 1);"));
}

#[test]
fn function_values_become_closures() {
    let c = common::compile(
        "fn twice(n: int) -> int { return n * 2 }
        fn apply(f: fn(int) -> int, x: int) -> int { return f(x) }
        fn main() { let f = twice print(apply(f, 5)) }",
    )
    .expect("compiles");
    // fn-typed parameters and locals are closure pointers
    assert!(c.contains("int64_t apply(void* f, int64_t x)"));
    assert!(c.contains("void* f;"));
    // a function reference builds a closure over an env-first wrapper
    assert!(c.contains("rune_closure_new((void *)rune_closure_twice, 0);"));
    assert!(c.contains("int64_t rune_closure_twice(void *env, int64_t n)"));
    // the wrapper forwards to the real function, dropping the environment
    assert!(c.contains("return twice(n);"));
}

#[test]
fn chained_calls_emit_nested_c_calls() {
    let c = generate(
        "fn pick(big: bool) -> fn(int) -> int { if big { return twice } return twice }
        fn twice(n: int) -> int { return n * 2 }
        fn main() { print(pick(true)(7)) }",
    );
    assert!(c.contains("rune_print_int(pick(true)(7))"));
}

#[test]
fn for_loops_become_c_for_loops() {
    let c = generate("fn main() { for let mut i = 0; i < 3; i += 1 { print(i) } }");
    assert!(c.contains("for (int64_t i = 0; (i < 3); i += 1) {"));
}

#[test]
fn break_and_continue_become_c_break_and_continue() {
    let c = generate("fn main() { while true { break continue } }");
    assert!(c.contains("break;"));
    assert!(c.contains("continue;"));
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

#[test]
fn arrays_use_the_runtime_helpers() {
    // Through the real IR pipeline: array literals, reads, writes, and len
    // all go through rune_arr_* helpers.
    let c = common::compile(
        "fn main() {
            let a = [1, 2, 3]
            print(a[1])
            let mut b = [4]
            b[0] += 5
            print(b[0])
            print(len(a))
        }",
    )
    .expect("compiles");
    // arrays are void* blocks; the literal allocates via the runtime
    assert!(c.contains("void* a;"));
    assert!(c.contains("= rune_arr_new(3);"));
    assert!(c.contains("rune_arr_set(t0, 0, (int64_t)(1));"));
    // reads narrow back; string/array elements get an explicit cast
    assert!(c.contains("= rune_arr_get(a, 1);"));
    assert!(c.contains("rune_arr_set(b, 0, (int64_t)("));
    assert!(c.contains("= rune_arr_len(a);"));
}

#[test]
fn pointer_captures_are_cast_back_in_wrappers() {
    // string and array captures are stored as i64 slots; the env-first
    // wrapper must cast them back to the parameter's pointer type.
    let c = common::compile(
        "fn main() {
            let s = \"hi\"
            let f = fn() -> string { return s }
            print(f())
            let a = [1]
            let g = fn() -> int { return a[0] }
            print(g())
        }",
    )
    .expect("compiles");
    assert!(c.contains("(const char*)((int64_t *)env)[1]"));
    assert!(c.contains("(void*)((int64_t *)env)[1]"));
}
