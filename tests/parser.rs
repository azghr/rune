// SPDX-License-Identifier: MIT

//! Parser tests: declarations, statements, precedence, and parser errors.

mod common;

use common::parse;
use rune::language::ast::{Expr, Stmt, Type};
use rune::language::token::TokenKind;

/// Returns the single function of a program that declares exactly one.
fn only_function(text: &str) -> rune::language::ast::Function {
    let program = parse(text);
    assert_eq!(program.functions.len(), 1, "expected one function");
    program.functions.into_iter().next().unwrap()
}

#[test]
fn parses_function_declarations() {
    let func = only_function("fn main() {}");
    assert_eq!(func.name, "main");
    assert!(func.params.is_empty());
    assert_eq!(func.return_type, None);
}

#[test]
fn parses_params_and_return_type() {
    let func = only_function("fn add(a: int, b: int) -> int { return a + b }");
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].name, "a");
    assert_eq!(func.params[0].ty, Type::Int);
    assert_eq!(func.params[1].name, "b");
    assert_eq!(func.return_type, Some(Type::Int));
}

#[test]
fn parses_let_with_and_without_annotation() {
    let func = only_function("fn main() { let a = 1 let b: bool = true }");
    match (&func.body.statements[0], &func.body.statements[1]) {
        (Stmt::Let { name, ty, .. }, Stmt::Let { ty: ty2, .. }) => {
            assert_eq!(name, "a");
            assert_eq!(*ty, None);
            assert_eq!(*ty2, Some(Type::Bool));
        }
        _ => panic!("expected let statements"),
    }
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    let func = only_function("fn main() { let x = 1 + 2 * 3 }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    let Expr::Binary { op, right, .. } = value else {
        panic!("expected binary expr");
    };
    assert_eq!(*op, TokenKind::Plus);
    assert!(matches!(
        right.as_ref(),
        Expr::Binary {
            op: TokenKind::Star,
            ..
        }
    ));
}

#[test]
fn subtraction_is_left_associative() {
    let func = only_function("fn main() { let x = 10 - 2 - 3 }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    let Expr::Binary { op, left, .. } = value else {
        panic!("expected binary expr");
    };
    assert_eq!(*op, TokenKind::Minus);
    assert!(matches!(
        left.as_ref(),
        Expr::Binary {
            op: TokenKind::Minus,
            ..
        }
    ));
}

#[test]
fn logical_operators_bind_looser_than_comparison() {
    let func = only_function("fn main() { let x = 1 < 2 && 3 < 4 }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    let Expr::Binary {
        op, left, right, ..
    } = value
    else {
        panic!("expected binary expr");
    };
    assert_eq!(*op, TokenKind::AmpAmp);
    assert!(matches!(
        left.as_ref(),
        Expr::Binary {
            op: TokenKind::Less,
            ..
        }
    ));
    assert!(matches!(
        right.as_ref(),
        Expr::Binary {
            op: TokenKind::Less,
            ..
        }
    ));
}

#[test]
fn parses_if_else() {
    let func = only_function("fn main() { if true { print(1) } else { print(2) } }");
    let Stmt::If { else_branch, .. } = &func.body.statements[0] else {
        panic!("expected if");
    };
    assert!(else_branch.is_some());
}

#[test]
fn parses_if_without_else() {
    let func = only_function("fn main() { if true { print(1) } }");
    let Stmt::If { else_branch, .. } = &func.body.statements[0] else {
        panic!("expected if");
    };
    assert!(else_branch.is_none());
}

#[test]
fn parses_else_if_chains() {
    let func = only_function(
        "fn main() { if true { print(1) } else if false { print(2) } else { print(3) } }",
    );
    let Stmt::If { else_branch, .. } = &func.body.statements[0] else {
        panic!("expected if");
    };
    // the else branch is a block holding exactly one nested if
    assert_eq!(else_branch.as_ref().unwrap().statements.len(), 1);
    assert!(matches!(
        &else_branch.as_ref().unwrap().statements[0],
        Stmt::If { .. }
    ));
}

#[test]
fn parses_while() {
    let func = only_function("fn main() { while true { print(1) } }");
    assert!(matches!(&func.body.statements[0], Stmt::While { .. }));
}

#[test]
fn parses_return_with_and_without_value() {
    let func = only_function("fn main() { return 1; return; }");
    match (&func.body.statements[0], &func.body.statements[1]) {
        (Stmt::Return { value: Some(_), .. }, Stmt::Return { value: None, .. }) => {}
        _ => panic!("expected two return statements"),
    }
}

/// A value-less `return` is recognized by the next token not being able to
/// start an expression — no explicit `;` is needed before `}`.
#[test]
fn bare_return_before_brace_parses_without_semicolon() {
    let func = only_function("fn main() { if true { return } print(1) }");
    let Stmt::If { then_branch, .. } = &func.body.statements[0] else {
        panic!("expected if");
    };
    let Stmt::Return { value, .. } = &then_branch.statements[0] else {
        panic!("expected return in then branch");
    };
    assert!(value.is_none());
}

/// A bare `return` followed by another statement also parses, with each
/// statement kept separate.
#[test]
fn bare_return_before_another_statement_parses() {
    let func = only_function("fn main() { return return }");
    assert_eq!(func.body.statements.len(), 2);
    assert!(matches!(
        &func.body.statements[0],
        Stmt::Return { value: None, .. }
    ));
    assert!(matches!(
        &func.body.statements[1],
        Stmt::Return { value: None, .. }
    ));
}

#[test]
fn parses_assignment() {
    let func = only_function("fn main() { let x = 1 x = 2 }");
    let Stmt::Assign { name, .. } = &func.body.statements[1] else {
        panic!("expected assignment");
    };
    assert_eq!(name, "x");
}

#[test]
fn parses_calls_with_nested_call_args() {
    let func = only_function("fn main() { let x = f(g(1), 2) }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    let Expr::Call { callee, args, .. } = value else {
        panic!("expected call");
    };
    assert_eq!(callee, "f");
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], Expr::Call { callee, .. } if callee == "g"));
}

#[test]
fn print_is_parsed_as_a_builtin_call() {
    let func = only_function("fn main() { print(1) }");
    let Stmt::Expr(Expr::Call { callee, .. }) = &func.body.statements[0] else {
        panic!("expected print call statement");
    };
    assert_eq!(callee, "print");
}

#[test]
fn parses_grouping_and_unary() {
    let func = only_function("fn main() { let x = -(1 + 2) }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    assert!(matches!(
        value,
        Expr::Unary {
            op: TokenKind::Minus,
            ..
        }
    ));
    let func = only_function("fn main() { let y = (1 + 2) * 3 }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    let Expr::Binary { left, .. } = value else {
        panic!("expected binary expr");
    };
    assert!(matches!(left.as_ref(), Expr::Grouping { .. }));
}

#[test]
fn errors_on_unterminated_block() {
    let err = common::parse_error("fn main() { let x = 1");
    assert!(err.message.contains("expected `}`"));
}

#[test]
fn errors_on_missing_equals_in_let() {
    let err = common::parse_error("fn main() { let x 1 }");
    assert!(err.message.contains("expected `=`"));
}

#[test]
fn errors_on_expected_expression() {
    let err = common::parse_error("fn main() { let x = }");
    assert!(err.message.contains("expected expression"));
}
