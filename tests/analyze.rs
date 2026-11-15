// SPDX-License-Identifier: MIT

//! Analyzer tests: scoping, type checking, and collected diagnostics.

mod common;

use common::{analyze, analyze_errors};

#[test]
fn accepts_a_valid_program() {
    let program = analyze(
        "fn main() {
            let x = 1
            let y: int = x + 2
            if y > 0 {
                print(y)
            } else {
                print(0)
            }
        }",
    );
    assert_eq!(program.functions.len(), 1);
}

#[test]
fn errors_on_undefined_variable_assignment() {
    let errs = analyze_errors("fn main() { x = 1 }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("undefined variable `x`"))
    );
}

#[test]
fn errors_on_let_annotation_mismatch() {
    let errs = analyze_errors("fn main() { let x: bool = 1 }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("expected `bool`, found `int`"))
    );
}

#[test]
fn errors_on_assignment_type_mismatch() {
    let errs = analyze_errors("fn main() { let mut x = 1 x = true }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot assign `bool`"))
    );
}

#[test]
fn errors_on_return_type_mismatch() {
    let errs = analyze_errors("fn f() -> int { return true }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("return type mismatch"))
    );
}

#[test]
fn errors_on_non_bool_if_condition() {
    let errs = analyze_errors("fn main() { if 1 { print(1) } }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("condition must be `bool`"))
    );
}

#[test]
fn errors_on_non_bool_while_condition() {
    let errs = analyze_errors("fn main() { while 1 { print(1) } }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("condition must be `bool`"))
    );
}

#[test]
fn errors_on_non_bool_logical_operands() {
    let errs = analyze_errors("fn main() { let x = 1 && 2 }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("operands must be bool"))
    );
}

#[test]
fn errors_on_arithmetic_on_bools() {
    let errs = analyze_errors("fn main() { let x = true + false }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("operands must be numbers"))
    );
}

#[test]
fn errors_on_equality_of_different_types() {
    let errs = analyze_errors("fn main() { let x = 1 == true }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("operands must have the same type"))
    );
}

#[test]
fn errors_on_duplicate_definition_in_same_scope() {
    let errs = analyze_errors("fn main() { let x = 1 let x = 2 }");
    assert!(errs.iter().any(|e| e.message.contains("already defined")));
}

#[test]
fn allows_shadowing_in_inner_scope() {
    analyze("fn main() { let x = 1 if true { let x = 2 print(x) } print(x) }");
}

#[test]
fn inner_scope_variables_are_not_visible_outside() {
    let errs = analyze_errors("fn main() { if true { let inner = 1 } print(inner) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("undefined variable `inner`"))
    );
}

#[test]
fn errors_on_print_arity() {
    let errs = analyze_errors("fn main() { print() }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("print takes exactly one argument"))
    );

    let errs = analyze_errors("fn main() { print(1, 2) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("print takes exactly one argument"))
    );
}

#[test]
fn string_equality_compares_contents() {
    // accepted by the checker; both C emitters compile it to strcmp
    analyze("fn main() { let x = \"a\" == \"b\" print(x) }");
}

#[test]
fn errors_on_string_compared_to_int() {
    let errs = analyze_errors("fn main() { let x = \"a\" == 1 }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("operands must have the same type"))
    );
}

#[test]
fn errors_on_binding_void_to_a_variable() {
    let errs = analyze_errors("fn f() { print(1) } fn main() { let x = f() }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot bind a `void` value"))
    );
}

#[test]
fn errors_on_printing_void() {
    let errs = analyze_errors("fn f() { print(1) } fn main() { print(f()) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot print a value of type `void`"))
    );
}

#[test]
fn string_concatenation_is_accepted() {
    analyze("fn main() { print(\"a\" + \"b\") let x = \"a\" + \"b\" print(x) }");
}

#[test]
fn errors_on_mixed_string_and_number_concatenation() {
    let errs = analyze_errors("fn main() { let x = \"a\" + 1 }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot mix strings and numbers"))
    );
}

#[test]
fn errors_on_string_arithmetic() {
    let errs = analyze_errors("fn main() { let x = \"a\" - \"b\" }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("operands must be numbers"))
    );
}

#[test]
fn compound_assignment_is_checked_like_the_expanded_form() {
    // `x += 1` on an int and `s += "t"` on a string are both fine
    analyze("fn main() { let mut x = 1 x += 1 let mut s = \"a\" s += \"b\" print(x) print(s) }");
}

#[test]
fn errors_on_compound_assignment_type_mismatch() {
    let errs = analyze_errors("fn main() { let mut x = 1 x += \"s\" }");
    assert!(errs.iter().any(|e| {
        e.message
            .contains("compound assignment requires two `int`s")
    }));
}

#[test]
fn errors_on_string_compound_subtraction() {
    let errs = analyze_errors("fn main() { let mut s = \"a\" s -= \"b\" }");
    assert!(errs.iter().any(|e| {
        e.message
            .contains("compound assignment requires two `int`s")
    }));
}

#[test]
fn errors_on_compound_assignment_to_immutable() {
    let errs = analyze_errors("fn main() { let x = 1 x += 1 }");
    assert!(errs.iter().any(|e| {
        e.message
            .contains("cannot assign to immutable variable `x`")
    }));
}

#[test]
fn break_and_continue_are_valid_inside_loops() {
    analyze("fn main() { while true { break } }");
    analyze("fn main() { while true { continue } }");
    analyze("fn main() { for ;; { break } }");
    analyze("fn main() { for ;; { continue } }");
}

#[test]
fn break_and_continue_nest_to_the_innermost_loop() {
    // both keywords inside the inner loop are fine, not an error
    analyze("fn main() { while true { while true { break continue } break } }");
}

#[test]
fn errors_on_break_outside_a_loop() {
    let errs = analyze_errors("fn main() { break }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("`break` outside of a loop"))
    );
}

#[test]
fn errors_on_continue_outside_a_loop() {
    let errs = analyze_errors("fn main() { continue }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("`continue` outside of a loop"))
    );
}

#[test]
fn functions_are_first_class_values() {
    // references, fn-typed parameters, and fn-typed returns all check
    analyze(
        "fn apply(f: fn(int) -> int, x: int) -> int { return f(x) }
        fn twice(n: int) -> int { return n * 2 }
        fn main() {
            let f = twice
            print(f(3))
            print(apply(twice, 4))
        }",
    );
}

#[test]
fn errors_on_calling_a_function_value_with_wrong_args() {
    let errs = analyze_errors(
        "fn twice(n: int) -> int { return n * 2 } fn main() { let f = twice f(1, 2) }",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("expects 1 argument(s), found 2"))
    );
}

#[test]
fn errors_on_fn_type_mismatch() {
    let errs = analyze_errors(
        "fn twice(n: int) -> int { return n * 2 } fn main() { let f: fn(int) -> int = 1 }",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("expected `fn(int) -> int`"))
    );
}

#[test]
fn errors_on_printing_a_function_value() {
    let errs =
        analyze_errors("fn twice(n: int) -> int { return n * 2 } fn main() { print(twice) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot print a value of type"))
    );
}

#[test]
fn collects_multiple_errors_across_statements() {
    let errs = analyze_errors("fn main() { let a: bool = 1 let b: int = true }");
    assert_eq!(errs.len(), 2);
}

#[test]
fn errors_on_call_to_undefined_function() {
    let errs = analyze_errors("fn main() { let x = mystery(1, 2) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("call to undefined function `mystery`"))
    );
}

#[test]
fn errors_on_wrong_argument_count() {
    let errs = analyze_errors(
        "fn add(a: int, b: int) -> int { return a + b } fn main() { print(add(1)) }",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("expects 2 argument(s), found 1"))
    );
}

#[test]
fn errors_on_wrong_argument_type() {
    let errs = analyze_errors("fn f(b: bool) -> int { return 1 } fn main() { print(f(1)) }");
    assert!(errs.iter().any(|e| {
        e.message
            .contains("argument type mismatch: expected `bool`, found `int`")
    }));
}

#[test]
fn errors_on_duplicate_function_names() {
    let errs = analyze_errors("fn f() { print(1) } fn f() { print(2) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("`f` is already defined"))
    );
}

#[test]
fn recursion_and_calls_before_definition_are_accepted() {
    // fib calls itself (recursion) and main calls fib before its definition
    // appears — signatures are collected before bodies are checked.
    analyze(
        "fn main() { print(fib(5)) }
        fn fib(n: int) -> int {
            if n < 2 { return n }
            return fib(n - 1) + fib(n - 2)
        }",
    );
}

#[test]
fn void_function_call_usable_as_a_statement() {
    analyze("fn f() { print(1) } fn main() { f() f() }");
}
