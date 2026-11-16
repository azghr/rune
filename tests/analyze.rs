// SPDX-License-Identifier: MIT

//! Analyzer tests: scoping, type checking, and collected diagnostics.

mod common;

use common::{analyze, analyze_errors, compile, parse};

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
fn chained_calls_check_against_the_returned_fn_type() {
    analyze(
        "fn pick(big: bool) -> fn(int) -> int {
            if big { return twice } return thrice
        }
        fn twice(n: int) -> int { return n * 2 }
        fn thrice(n: int) -> int { return n * 3 }
        fn main() { print(pick(true)(7)) print(pick(false)(7)) }",
    );
}

#[test]
fn errors_on_calling_a_non_function() {
    let errs = analyze_errors("fn main() { let x = 1 x(2) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot call a value of type `int`"))
    );
}

#[test]
fn anonymous_functions_type_check() {
    analyze(
        "fn apply(f: fn(int) -> int, x: int) -> int { return f(x) }
        fn main() {
            let add = fn(a: int, b: int) -> int { return a + b }
            print(add(1, 2))
            print(apply(fn(n: int) -> int { return n * 10 }, 4))
        }",
    );
}

#[test]
fn lambda_return_type_mismatches_are_errors() {
    let errs = analyze_errors("fn main() { let f = fn() -> int { return true } print(f()) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("return type mismatch"))
    );
}

#[test]
fn captures_type_check_against_the_captured_variable() {
    // by-value captures: reads inside the lambda check against the outer
    // variable's type
    analyze(
        "fn main() {
            let n = 5
            let addn = fn(x: int) -> int { return x + n }
            print(addn(1))
        }",
    );
}

#[test]
fn errors_on_assigning_to_a_captured_variable() {
    let errs = analyze_errors(
        "fn main() {
            let mut n = 5
            let bump = fn(x: int) -> int { n = n + x return n }
            print(bump(1))
        }",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot assign to captured variable `n`"))
    );
}

#[test]
fn lambda_params_shadow_nothing_and_arity_is_checked() {
    let errs =
        analyze_errors("fn main() { let f = fn(x: int) -> int { return x } print(f(1, 2)) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("expects 1 argument(s), found 2"))
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

#[test]
fn accepts_well_typed_arrays() {
    let program = analyze(
        "fn main() {
            let a: [int] = [1, 2, 3]
            let s = [\"x\", \"y\"]
            print(a[0])
            print(s[1])
            print(len(a))
        }",
    );
    assert_eq!(program.functions.len(), 1);
}

#[test]
fn errors_on_mixed_array_element_types() {
    let errs = analyze_errors("fn main() { let a = [1, \"x\"] }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("array elements must have the same type"))
    );
}

#[test]
fn errors_on_empty_array_literal() {
    let errs = analyze_errors("fn main() { let a = [] }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("empty array literals are not supported"))
    );
}

#[test]
fn errors_on_index_type_mismatch() {
    let errs = analyze_errors("fn main() { let a = [1, 2] print(a[\"x\"]) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("array index must be `int`"))
    );
}

#[test]
fn errors_on_indexing_a_non_array() {
    let errs = analyze_errors("fn main() { let n = 3 print(n[0]) }");
    assert!(errs.iter().any(|e| {
        e.message
            .contains("cannot index into a value of type `int`")
    }));
}

#[test]
fn errors_on_assigning_wrong_element_type() {
    let errs = analyze_errors("fn main() { let a = [1, 2] a[0] = \"x\" }");
    assert!(errs.iter().any(|e| {
        e.message
            .contains("cannot assign `string` to array element of type `int`")
    }));
}

#[test]
fn errors_on_array_comparison() {
    let errs = analyze_errors("fn main() { let a = [1] let b = [2] print(a == b) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("arrays cannot be compared"))
    );
}

#[test]
fn errors_on_len_of_non_array() {
    let errs = analyze_errors("fn main() { print(len(3)) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("len expects an array, found `int`"))
    );
}

#[test]
fn errors_on_printing_an_array() {
    let errs = analyze_errors("fn main() { let a = [1] print(a) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot print a value of type `[int]`"))
    );
}

#[test]
fn arrays_can_be_passed_and_returned() {
    let program = analyze(
        "fn sum(a: [int]) -> int { return a[0] }
        fn make(n: int) -> [int] { return [n, n] }
        fn main() { print(sum([4, 5])) print(make(9)[1]) }",
    );
    assert_eq!(program.functions.len(), 3);
}

#[test]
fn errors_on_duplicate_parameters() {
    let errs = analyze_errors("fn f(a: int, a: string) -> int { return a } fn main() { }");
    assert!(
        errs.iter().any(|e| e.message.contains("already defined")),
        "expected a duplicate-parameter error, got: {:?}",
        errs
    );
}

#[test]
fn errors_on_main_with_parameters() {
    let errs = analyze_errors("fn main(a: int) { print(a) }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("`main` must not take parameters")),
        "expected a main-signature error, got: {:?}",
        errs
    );
}

#[test]
fn errors_on_break_inside_an_anonymous_function_in_a_loop() {
    let errs = analyze_errors(
        "fn main() { let mut i = 0 while i < 3 { let f = fn() { break } f() i += 1 } }",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("`break` outside of a loop")),
        "expected a break-outside-loop error, got: {:?}",
        errs
    );
}

#[test]
fn errors_on_compound_assignment_to_a_captured_variable() {
    let errs = analyze_errors("fn main() { let mut x = 1 let f = fn() { x += 1 } f() }");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("cannot assign to captured variable")),
        "expected a captured-assignment error, got: {:?}",
        errs
    );
}

#[test]
fn errors_on_missing_main_when_an_executable_entry_is_required() {
    use rune::language::analyze::Analyzer;
    let errs = Analyzer::new()
        .require_main(true)
        .analyze(parse("fn helper() { print(1) }"))
        .expect_err("analysis should fail without `main`");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("program has no `main` function")),
        "expected a missing-`main` error, got: {:?}",
        errs
    );
}

#[test]
fn accepts_a_missing_main_when_not_required() {
    use rune::language::analyze::Analyzer;
    // Library modules (e.g. the `--wat` pipeline) may omit `main`.
    Analyzer::new()
        .analyze(parse("fn helper() { print(1) }"))
        .expect("analysis should succeed without `main` when not required");
}

#[test]
fn compile_requires_a_main_function() {
    let result = compile("fn helper() { print(1) }");
    assert!(
        result.is_err() && result.unwrap_err().contains("compilation failed"),
        "compiling an executable without `main` should fail"
    );
}

#[test]
fn errors_when_a_non_void_function_can_fall_off_the_end() {
    let errs = analyze_errors(
        "fn f(n: int) -> int {
            if n > 0 { return 1 }
        }
        fn main() { print(f(1)) }",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("not all paths return a value")),
        "expected a missing-return error, got: {:?}",
        errs
    );
}

#[test]
fn accepts_a_function_that_returns_on_every_path() {
    analyze(
        "fn f(n: int) -> int {
            if n > 0 { return 1 } else { return 2 }
        }
        fn main() { print(f(1)) }",
    );
}

#[test]
fn empty_array_literal_reports_one_error_without_a_false_mismatch() {
    let errs = analyze_errors("fn main() { let a: [int] = [] }");
    assert_eq!(
        errs.iter()
            .filter(|e| e.message.contains("empty array"))
            .count(),
        1,
        "expected exactly the empty-array error, got: {:?}",
        errs
    );
    assert!(
        !errs.iter().any(|e| e.message.contains("type mismatch")),
        "empty-array recovery must not cascade, got: {:?}",
        errs
    );
}
