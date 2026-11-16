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
    assert!(matches!(callee.as_ref(), Expr::Variable { name, .. } if name == "f"));
    assert_eq!(args.len(), 2);
    assert!(matches!(
        &args[0],
        Expr::Call { callee, .. }
            if matches!(callee.as_ref(), Expr::Variable { name, .. } if name == "g")
    ));
}

#[test]
fn print_is_parsed_as_a_builtin_call() {
    let func = only_function("fn main() { print(1) }");
    let Stmt::Expr(Expr::Call { callee, .. }) = &func.body.statements[0] else {
        panic!("expected print call statement");
    };
    assert!(matches!(callee.as_ref(), Expr::Variable { name, .. } if name == "print"));
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
fn parses_compound_assignment() {
    let func = only_function("fn main() { let mut x = 1 x += 2 x -= 1 x *= 3 x /= 2 x %= 4 }");
    let ops: Vec<TokenKind> = func
        .body
        .statements
        .iter()
        .filter_map(|s| match s {
            Stmt::Assign { op, .. } => Some(op.clone()),
            _ => None,
        })
        .collect();
    // the `let` binding is a Let statement, so only the five compound
    // assignments appear
    assert_eq!(
        ops,
        vec![
            TokenKind::PlusEq,
            TokenKind::MinusEq,
            TokenKind::StarEq,
            TokenKind::SlashEq,
            TokenKind::PercentEq,
        ]
    );
}

/// The `for` loop is pure syntax sugar: it desugars into a block holding
/// the initializer followed by a `while` whose body ends with the step.
#[test]
fn parses_for_loop_clauses() {
    let func = only_function("fn main() { for let mut i = 0; i < 3; i += 1 { print(i) } }");
    let Stmt::For {
        init,
        condition,
        step,
        body,
        ..
    } = &func.body.statements[0]
    else {
        panic!("expected a for statement");
    };
    assert!(matches!(init.as_deref(), Some(Stmt::Let { .. })));
    assert!(matches!(condition, Some(Expr::Binary { .. })));
    assert!(matches!(
        step.as_deref(),
        Some(Stmt::Assign {
            op: TokenKind::PlusEq,
            ..
        })
    ));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn for_loop_keeps_its_own_ast_node() {
    // `for` is NOT desugared into `while`: `continue` in a for loop must
    // reach the step clause, which only a dedicated node can express.
    let func = only_function("fn main() { for ;; { continue } }");
    assert!(matches!(&func.body.statements[0], Stmt::For { .. }));
}

#[test]
fn parses_chained_calls() {
    let func = only_function("fn main() { print(pick(true)(7)) }");
    let Stmt::Expr(Expr::Call { args, .. }) = &func.body.statements[0] else {
        panic!("expected call statement");
    };
    assert_eq!(args.len(), 1);
    // print's argument is the chained call: pick(true)(7)
    let Expr::Call {
        callee,
        args: inner_args,
        ..
    } = &args[0]
    else {
        panic!("expected a chained call");
    };
    assert!(matches!(callee.as_ref(), Expr::Call { .. }));
    assert_eq!(inner_args.len(), 1);
}

#[test]
fn parses_anonymous_functions() {
    let func = only_function(
        "fn main() { let f = fn(a: int, b: int) -> int { return a + b } print(f(1, 2)) }",
    );
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    let Expr::AnonymousFunction {
        params,
        return_type,
        body,
        ..
    } = value
    else {
        panic!("expected an anonymous function");
    };
    assert_eq!(params.len(), 2);
    assert_eq!(*return_type, Some(Type::Int));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn parses_break_and_continue() {
    let func = only_function("fn main() { while true { break continue } }");
    let Stmt::While { body, .. } = &func.body.statements[0] else {
        panic!("expected while");
    };
    assert!(matches!(&body.statements[0], Stmt::Break { .. }));
    assert!(matches!(&body.statements[1], Stmt::Continue { .. }));
}

#[test]
fn for_loop_clauses_are_optional() {
    // all clauses empty: an infinite loop with a synthesized `true` condition
    let func = only_function("fn main() { for ;; { return } }");
    let Stmt::For {
        init,
        condition,
        step,
        ..
    } = &func.body.statements[0]
    else {
        panic!("expected a for statement");
    };
    assert!(init.is_none());
    assert!(condition.is_none());
    assert!(step.is_none());
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

#[test]
fn parses_array_types() {
    let func = only_function("fn f(a: [int], b: [[string]]) {}");
    assert_eq!(func.params[0].ty, Type::Array(Box::new(Type::Int)));
    assert_eq!(
        func.params[1].ty,
        Type::Array(Box::new(Type::Array(Box::new(Type::String))))
    );
}

#[test]
fn parses_array_literals_and_indexing() {
    let func = only_function("fn main() { let a = [1, 2, 3] print(a[1]) }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected a let");
    };
    let Expr::ArrayLiteral { elems, .. } = value else {
        panic!("expected an array literal");
    };
    assert_eq!(elems.len(), 3);

    let Stmt::Expr(expr) = &func.body.statements[1] else {
        panic!("expected an expression statement");
    };
    let Expr::Call { args, .. } = expr else {
        panic!("expected a print call");
    };
    assert!(matches!(&args[0], Expr::Index { .. }));
}

#[test]
fn parses_index_assignment_and_chained_indexing() {
    let func = only_function("fn main() { let mut a = [1] a[0] += 2 let m = [[1]] m[0][0] = 7 }");
    assert!(matches!(
        &func.body.statements[1],
        Stmt::IndexAssign {
            op: TokenKind::PlusEq,
            ..
        }
    ));
    assert!(matches!(
        &func.body.statements[3],
        Stmt::IndexAssign {
            op: TokenKind::Eq,
            ..
        }
    ));
}

#[test]
fn parses_indexing_on_call_results() {
    let func = only_function("fn main() { print(make(1)[0]) }");
    let Stmt::Expr(expr) = &func.body.statements[0] else {
        panic!("expected an expression statement");
    };
    let Expr::Call { args, .. } = expr else {
        panic!("expected a print call");
    };
    let Expr::Index { array, .. } = &args[0] else {
        panic!("expected an index expression");
    };
    assert!(matches!(array.as_ref(), Expr::Call { .. }));
}

#[test]
fn array_literal_requires_closing_bracket() {
    let err = common::parse_error("fn main() { let a = [1, 2 }");
    assert!(err.message.contains("expected `]`"));
}

#[test]
fn and_binds_tighter_than_or() {
    // `a || b && c` must parse as `a || (b && c)`.
    let func = only_function("fn main() { let x = true || false && true }");
    let Stmt::Let { value, .. } = &func.body.statements[0] else {
        panic!("expected let");
    };
    let Expr::Binary { op, right, .. } = value else {
        panic!("expected binary expr");
    };
    assert_eq!(*op, TokenKind::PipePipe);
    assert!(matches!(
        right.as_ref(),
        Expr::Binary {
            op: TokenKind::AmpAmp,
            ..
        }
    ));
}
