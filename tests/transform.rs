// SPDX-License-Identifier: MIT

//! Tests for AST-to-IR lowering and the transform passes.

mod common;

use common::analyze;
use rune::language::Type;
use rune::transform::ir::*;
use rune::transform::{ConstFold, DeadCodeElim, lower_program};

fn lower(text: &str) -> Module {
    lower_program(&analyze(text))
}

fn lowered(text: &str) -> Module {
    let mut module = lower(text);
    ConstFold::run(&mut module);
    DeadCodeElim::run(&mut module);
    module
}

#[test]
fn straight_line_code_is_one_block() {
    let module = lower("fn main() { let x = 1 print(x) }");
    assert_eq!(module.functions.len(), 1);
    let body = &module.functions[0].body;
    assert_eq!(body.len(), 1);
    assert!(matches!(body[0].terminator, Terminator::Return(None)));
}

#[test]
fn if_lowering_produces_branch_and_merge_blocks() {
    let module = lower("fn main() { if true { print(1) } else { print(2) } }");
    let body = &module.functions[0].body;
    // entry (branch) + then + else + merge
    assert_eq!(body.len(), 4);
    assert!(matches!(body[0].terminator, Terminator::Branch { .. }));
    assert!(matches!(body[1].terminator, Terminator::Jump(3)));
    assert!(matches!(body[2].terminator, Terminator::Jump(3)));
    assert!(matches!(body[3].terminator, Terminator::Return(None)));
}

#[test]
fn while_lowering_produces_back_edge() {
    let module = lower("fn main() { while true { print(1) } }");
    let body = &module.functions[0].body;
    // entry (jump to cond) + cond (branch) + body (back edge) + exit
    assert_eq!(body.len(), 4);
    assert!(matches!(body[0].terminator, Terminator::Jump(1)));
    assert!(matches!(body[1].terminator, Terminator::Branch { .. }));
    // the body jumps back to the condition block
    assert!(matches!(body[2].terminator, Terminator::Jump(1)));
    assert!(matches!(body[3].terminator, Terminator::Return(None)));
}

#[test]
fn logical_and_lowers_to_short_circuit_control_flow() {
    // `g()` must only run when `f()` returned true, so both calls need to
    // live in different basic blocks connected by a Branch.
    let module = lower(
        "fn main() { let x = f() && g() } fn f() -> bool { return true } fn g() -> bool { return false }",
    );
    let body = &module.functions[0].body;
    let branches = body
        .iter()
        .filter(|b| matches!(b.terminator, Terminator::Branch { .. }))
        .count();
    assert!(branches >= 1, "&& must lower to a Branch, blocks: {body:?}");
    // the two calls are in different blocks
    let call_blocks = body
        .iter()
        .filter(|b| {
            b.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Call { dest: Some(_), .. }))
        })
        .count();
    assert_eq!(call_blocks, 2, "f and g must be in separate blocks");
}

#[test]
fn const_fold_folds_arithmetic() {
    let module = lowered("fn main() { let x = 2 * 3 + 4 print(x) }");
    let main = &module.functions[0];
    // `x` is bound directly to the folded constant
    assert!(main.body[0].instructions.iter().any(|i| matches!(
        i,
        Instruction::Move { dest, value: Value::ConstInt(10), .. } if dest == "x"
    )));
}

#[test]
fn const_fold_survives_division_by_zero() {
    let module = lowered("fn main() { let x = 1 / 0 print(x) }");
    let main = &module.functions[0];
    // not folded, but the program still contains the division
    assert!(
        main.body[0]
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::Binary { op: BinOp::Div, .. }))
    );
}

#[test]
fn dead_code_elim_removes_unused_bindings() {
    let module = lowered("fn main() { let mut x = 1 let y = x + 5 print(x) }");
    let main = &module.functions[0];
    // `y` is never used, so its whole temp chain disappears
    assert!(
        main.body[0]
            .instructions
            .iter()
            .all(|i| !matches!(i, Instruction::Binary { dest, .. } if dest == "y"))
    );
    // `x` is used and stays
    assert!(main.body[0].instructions.iter().any(
        |i| matches!(i, Instruction::Move { dest, value: Value::ConstInt(1), .. } if dest == "x")
    ));
}

#[test]
fn dead_code_elim_keeps_loop_counters_across_blocks() {
    // `i` is assigned in the entry block but used in the condition block —
    // a per-block-only analysis would wrongly remove it.
    let module = lowered("fn main() { let mut i = 0 while i < 3 { i = i + 1 } print(i) }");
    let main = &module.functions[0];
    assert!(main.body.iter().any(|b| b.instructions.iter().any(
        |i| matches!(i, Instruction::Move { dest, value: Value::ConstInt(0), .. } if dest == "i")
    )));
}

#[test]
fn calls_are_never_eliminated() {
    let module = lowered("fn f() -> int { return 1 } fn main() { f() }");
    let main = module.functions.last().unwrap();
    assert!(
        main.body[0]
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::Call { name, .. } if name == "f"))
    );
}

#[test]
fn temp_names_never_collide_with_user_variables() {
    let module = lower("fn main() { let t0 = 1 let t1 = 2 print(t0 + t1) }");
    let main = &module.functions[0];
    let mut names = vec![];
    for block in &main.body {
        for inst in &block.instructions {
            if let Instruction::Binary { dest, .. } = inst {
                names.push(dest.clone());
            }
        }
    }
    // exactly one temp was generated and it skipped over the user's names
    assert_eq!(names.len(), 1, "expected exactly one temp, got {names:?}");
    assert_ne!(names[0], "t0");
    assert_ne!(names[0], "t1");
}

#[test]
fn string_equality_lowering_keeps_eq_operators() {
    let module = lowered("fn main() { let x = \"a\" == \"b\" print(x) }");
    let main = &module.functions[0];
    assert!(
        main.body[0]
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::Binary { op: BinOp::Eq, .. }))
    );
}

#[test]
fn constant_branches_are_folded_to_jumps() {
    let module = lowered("fn main() { if true { print(1) } else { print(2) } }");
    let body = &module.functions[0].body;
    // entry jumps straight to the then block; the dead else block is gone
    assert!(matches!(body[0].terminator, Terminator::Jump(1)));
    assert!(
        body.iter()
            .all(|b| !matches!(b.terminator, Terminator::Branch { .. }))
    );
    assert_eq!(body.len(), 3, "dead else block should be removed: {body:?}");
}

#[test]
fn constant_variables_fold_branches_via_terminator_propagation() {
    let module = lowered("fn main() { let flag = true if flag { print(1) } else { print(2) } }");
    let body = &module.functions[0].body;
    assert!(
        body.iter()
            .all(|b| !matches!(b.terminator, Terminator::Branch { .. }))
    );
}

#[test]
fn while_false_removes_the_loop_body() {
    let module = lowered("fn main() { while false { print(1) } print(2) }");
    let body = &module.functions[0].body;
    // entry jumps to cond, cond jumps to exit, body removed
    assert!(!body.iter().any(|b| b.instructions.iter().any(
        |i| matches!(i, Instruction::Call { name, .. } if name == "print" && {
            if let Instruction::Call { args, .. } = i {
                matches!(args.first(), Some(Value::ConstInt(1)))
            } else {
                false
            }
        })
    )));
    assert!(
        body.iter()
            .all(|b| !matches!(b.terminator, Terminator::Branch { .. }))
    );
}

#[test]
fn real_branches_survive_branch_folding() {
    // both sides depend on runtime data, so nothing may be folded
    let module = lowered(include_str!("../examples/fib.rune"));
    assert!(
        module
            .functions
            .iter()
            .flat_map(|f| f.body.iter())
            .any(|b| matches!(b.terminator, Terminator::Branch { .. }))
    );
}

#[test]
fn dead_stores_overwritten_before_read_are_removed() {
    let module = lowered("fn main() { let mut x = 1 x = 2 print(x) }");
    let main = &module.functions[0];
    assert!(!main.body[0].instructions.iter().any(
        |i| matches!(i, Instruction::Move { dest, value: Value::ConstInt(1), .. } if dest == "x")
    ));
    // the live store stays
    assert!(main.body[0].instructions.iter().any(
        |i| matches!(i, Instruction::Move { dest, value: Value::ConstInt(2), .. } if dest == "x")
    ));
}

#[test]
fn unused_temp_chains_are_fully_removed_by_liveness() {
    let module = lowered("fn main() { let dead = 1 + 2 print(3) }");
    let main = &module.functions[0];
    assert!(
        main.body[0]
            .instructions
            .iter()
            .all(|i| !matches!(i, Instruction::Binary { dest, .. } if dest == "dead"))
    );
}

#[test]
fn string_concatenation_keeps_string_type_in_ir() {
    let module = lowered("fn main() { let x = \"a\" + \"b\" print(x) }");
    let main = &module.functions[0];
    assert!(main.body[0].instructions.iter().any(|i| matches!(
        i,
        Instruction::Binary {
            op: BinOp::Add,
            ty: Type::String,
            ..
        }
    )));
}

#[test]
fn comparing_a_concatenation_yields_bool_in_ir() {
    let module = lowered("fn main() { print(\"a\" + \"b\" == \"ab\") }");
    let main = &module.functions[0];
    assert!(main.body[0].instructions.iter().any(|i| matches!(
        i,
        Instruction::Binary {
            op: BinOp::Eq,
            ty: Type::Bool,
            ..
        }
    )));
}

#[test]
fn compound_assignment_lowers_to_binary_plus_move() {
    // pre-pass lowering: `1 + 2` would be constant-folded away otherwise
    let module = lower("fn main() { let mut x = 1 x += 2 print(x) }");
    let main = &module.functions[0];
    let insts = &main.body[0].instructions;
    assert!(insts.iter().any(|i| matches!(
        i,
        Instruction::Binary {
            op: BinOp::Add,
            ty: Type::Int,
            ..
        }
    )));
    assert!(insts.iter().any(|i| matches!(
        i,
        Instruction::Move { dest, ty: Type::Int, .. } if dest == "x"
    )));
}

#[test]
fn string_compound_assignment_lowers_as_concatenation() {
    let module = lowered("fn main() { let mut s = \"a\" s += \"b\" print(s) }");
    let main = &module.functions[0];
    assert!(main.body[0].instructions.iter().any(|i| matches!(
        i,
        Instruction::Binary {
            op: BinOp::Add,
            ty: Type::String,
            ..
        }
    )));
}

#[test]
fn assigning_a_non_constant_invalidates_known_constants() {
    // Regression: propagation once kept `x -> 1` live across `x = f()`,
    // folding `print(x)` into `print(1)`.
    let module = lowered("fn f() -> int { return 7 } fn main() { let mut x = 1 x = f() print(x) }");
    let main = module.functions.last().unwrap();
    let print_args = main
        .body
        .iter()
        .flat_map(|b| b.instructions.iter())
        .filter_map(|i| match i {
            Instruction::Call { name, args, .. } if name == "print" => args.first(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(print_args.len(), 1);
    assert!(
        !matches!(print_args[0], Value::ConstInt(1)),
        "the stale constant must not survive the non-constant store"
    );
}

#[test]
fn break_lowers_to_a_jump_to_the_loop_exit() {
    let module = lowered("fn main() { while true { break } }");
    let body = &module.functions[0].body;
    // entry -> cond -> body -> exit; the break block jumps to the exit
    let exit = body.len() - 1;
    assert!(
        body.iter()
            .any(|b| matches!(b.terminator, Terminator::Jump(t) if t == exit))
    );
}

#[test]
fn continue_in_while_jumps_to_the_condition() {
    let module = lowered("fn main() { let mut i = 0 while i < 3 { i += 1 continue } }");
    let body = &module.functions[0].body;
    // the cond block is block 1 (entry jumps to it); some block must jump back
    assert!(
        body.iter()
            .any(|b| matches!(b.terminator, Terminator::Jump(1)))
    );
}

#[test]
fn continue_in_for_runs_the_step_clause() {
    // C semantics: `continue` must NOT skip `i += 1`, or the loop would not
    // terminate. The IR routes `continue` into the step block.
    let module = lowered("fn main() { for let mut i = 0; i < 3; i += 1 { continue } }");
    let body = &module.functions[0].body;
    // the step block holds the `i + 1` computation and jumps back to the
    // condition block (block 1, since the entry jumps to the condition)
    let step_block = body
        .iter()
        .find(|b| {
            b.instructions.iter().any(|i| {
                matches!(
                    i,
                    Instruction::Binary {
                        left: Value::Local(n),
                        op: BinOp::Add,
                        right: Value::ConstInt(1),
                        ..
                    } if n == "i"
                )
            })
        })
        .expect("step block with `i += 1` should exist");
    assert!(
        matches!(step_block.terminator, Terminator::Jump(1)),
        "step must jump back to the condition block, got {:?}",
        step_block.terminator
    );
    // and the loop body routes `continue` into the step block
    let step_id = step_block.id;
    assert!(
        body.iter()
            .any(|b| matches!(b.terminator, Terminator::Jump(t) if t == step_id))
    );
}

#[test]
fn calls_through_function_values_lower_to_call_indirect() {
    let module =
        lowered("fn twice(n: int) -> int { return n * 2 } fn main() { let f = twice print(f(3)) }");
    let main = module.functions.last().unwrap();
    let insts: Vec<&Instruction> = main
        .body
        .iter()
        .flat_map(|b| b.instructions.iter())
        .collect();
    assert!(insts.iter().any(
        |i| matches!(i, Instruction::CallIndirect { callee: Value::Local(n), .. } if n == "f")
    ));
    // binding a function reference is a Move of the function's name
    assert!(insts.iter().any(|i| matches!(
        i,
        Instruction::Move { dest, value: Value::Local(v), ty: Type::Function { .. } }
            if dest == "f" && v == "twice"
    )));
}

#[test]
fn programs_through_the_full_pass_pipeline_stay_equivalent() {
    // Just verify lowering + passes don't crash on representative programs;
    // end-to-end behavior is covered by the integration tests.
    lowered(include_str!("../examples/fib.rune"));
    lowered(include_str!("../examples/hello.rune"));
}
