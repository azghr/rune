// SPDX-License-Identifier: MIT

//! Conservative dead code elimination.
//!
//! Removes assignments whose results are never used anywhere in the
//! function. Calls are always kept because they may have side effects.
//! The used-set is computed for the whole function so locals that live
//! across basic blocks (loop counters, values read in later blocks) are
//! preserved; this is intentionally conservative rather than a precise
//! liveness analysis.

use crate::transform::ir::*;
use std::collections::HashSet;

pub struct DeadCodeElim;

impl DeadCodeElim {
    pub fn run(module: &mut Module) {
        for func in &mut module.functions {
            // Collect every local read anywhere in the function.
            let mut used = HashSet::new();
            for block in &func.body {
                Self::collect_uses(&block.instructions, &mut used);
                Self::collect_terminator_uses(&block.terminator, &mut used);
            }
            for block in &mut func.body {
                Self::eliminate_in_block(block, &used);
            }
        }
    }

    fn collect_uses(instructions: &[Instruction], used: &mut HashSet<String>) {
        for inst in instructions {
            match inst {
                Instruction::Binary { left, right, .. } => {
                    Self::collect_value_uses(left, used);
                    Self::collect_value_uses(right, used);
                }
                Instruction::Unary { value, .. } | Instruction::Move { value, .. } => {
                    Self::collect_value_uses(value, used);
                }
                Instruction::Call { args, .. } => {
                    for arg in args {
                        Self::collect_value_uses(arg, used);
                    }
                }
            }
        }
    }

    fn collect_terminator_uses(terminator: &Terminator, used: &mut HashSet<String>) {
        match terminator {
            Terminator::Return(Some(value)) => Self::collect_value_uses(value, used),
            Terminator::Branch { condition, .. } => Self::collect_value_uses(condition, used),
            _ => {}
        }
    }

    fn collect_value_uses(value: &Value, used: &mut HashSet<String>) {
        if let Value::Local(name) = value {
            used.insert(name.clone());
        }
    }

    fn eliminate_in_block(block: &mut BasicBlock, used: &HashSet<String>) {
        block.instructions.retain(|inst| {
            let dest = match inst {
                Instruction::Binary { dest, .. }
                | Instruction::Unary { dest, .. }
                | Instruction::Move { dest, .. } => Some(dest),
                Instruction::Call { dest: _, .. } => {
                    // Calls may have side effects; always keep them.
                    return true;
                }
            };
            match dest {
                Some(dest) => used.contains(dest),
                None => true,
            }
        });
    }
}
