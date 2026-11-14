//! Very simple dead code elimination.
//!
//! Removes instructions whose results are never used.
//! This is a basic version that only works on a single basic block
//! and does not yet handle control flow.

use crate::transform::ir::*;
use std::collections::HashSet;

pub struct DeadCodeElim;

impl DeadCodeElim {
    pub fn run(module: &mut Module) {
        for func in &mut module.functions {
            for block in &mut func.body {
                Self::eliminate_in_block(block);
            }
        }
    }

    fn eliminate_in_block(block: &mut BasicBlock) {
        let mut used = HashSet::new();

        // Collect used locals from the terminator
        match &block.terminator {
            Terminator::Return(Some(Value::Local(name))) => {
                used.insert(name.clone());
            }
            Terminator::Branch {
                condition: Value::Local(name),
                ..
            } => {
                used.insert(name.clone());
            }
            _ => {}
        }

        // Walk instructions backwards
        let mut new_instructions = Vec::new();
        for inst in block.instructions.iter().rev() {
            let (dest, is_useful) = match inst {
                Instruction::Binary {
                    dest, left, right, ..
                } => {
                    let useful = used.contains(dest);
                    if useful {
                        if let Value::Local(n) = left {
                            used.insert(n.clone());
                        }
                        if let Value::Local(n) = right {
                            used.insert(n.clone());
                        }
                    }
                    (dest.clone(), useful)
                }
                Instruction::Unary { dest, value, .. } => {
                    let useful = used.contains(dest);
                    if useful {
                        if let Value::Local(n) = value {
                            used.insert(n.clone());
                        }
                    }
                    (dest.clone(), useful)
                }
                Instruction::Move { dest, value, .. } => {
                    let useful = used.contains(dest);
                    if useful {
                        if let Value::Local(n) = value {
                            used.insert(n.clone());
                        }
                    }
                    (dest.clone(), useful)
                }
                Instruction::Call { dest, args, .. } => {
                    // Calls are side-effecting → always keep
                    for arg in args {
                        if let Value::Local(n) = arg {
                            used.insert(n.clone());
                        }
                    }
                    (dest.clone().unwrap_or_default(), true)
                }
            };

            if is_useful {
                new_instructions.push(inst.clone());
            }
        }

        new_instructions.reverse();
        block.instructions = new_instructions;
    }
}
