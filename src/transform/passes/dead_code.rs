// SPDX-License-Identifier: MIT

//! Dead code elimination based on backward liveness analysis.
//!
//! An assignment is removed when its destination is dead: not read by any
//! instruction or terminator that executes after it on any path. Liveness is
//! computed with a standard backward dataflow fixpoint over the whole
//! control-flow graph, so values that live across blocks and loop-carried
//! variables are preserved, while dead stores (including values overwritten
//! before ever being read) are removed. Calls are always kept because they
//! may have side effects.

use crate::transform::ir::*;
use std::collections::{HashMap, HashSet};

pub struct DeadCodeElim;

impl DeadCodeElim {
    pub fn run(module: &mut Module) {
        for func in &mut module.functions {
            let live_in = Self::liveness(func);
            for block in &mut func.body {
                Self::eliminate_in_block(block, &live_in);
            }
        }
    }

    /// Backward liveness fixpoint: returns the set of locals live at the
    /// entry of each block.
    fn liveness(func: &Function) -> HashMap<usize, HashSet<String>> {
        let mut live_in: HashMap<usize, HashSet<String>> =
            func.body.iter().map(|b| (b.id, HashSet::new())).collect();

        loop {
            let mut changed = false;
            for block in func.body.iter().rev() {
                // live-out = union of successors' live-in
                let mut live = HashSet::new();
                for succ in Self::successors(&block.terminator) {
                    live.extend(live_in[&succ].iter().cloned());
                }

                // The terminator itself uses its condition/return value.
                for used in Self::terminator_uses(&block.terminator) {
                    live.insert(used);
                }

                // Walk instructions backwards: use kills defs.
                for inst in block.instructions.iter().rev() {
                    for used in Self::used_by(inst) {
                        live.insert(used);
                    }
                    if let Some(dest) = Self::dest_of(inst) {
                        live.remove(&dest);
                    }
                }

                let entry = live_in.get_mut(&block.id).unwrap();
                if *entry != live {
                    *entry = live;
                    changed = true;
                }
            }
            if !changed {
                return live_in;
            }
        }
    }

    fn successors(terminator: &Terminator) -> Vec<usize> {
        match terminator {
            Terminator::Return(_) => vec![],
            Terminator::Jump(target) => vec![*target],
            Terminator::Branch {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
        }
    }

    fn dest_of(inst: &Instruction) -> Option<String> {
        match inst {
            Instruction::Binary { dest, .. }
            | Instruction::Unary { dest, .. }
            | Instruction::Move { dest, .. } => Some(dest.clone()),
            Instruction::Call { .. } => None,
        }
    }

    fn used_by(inst: &Instruction) -> Vec<String> {
        let mut uses = Vec::new();
        match inst {
            Instruction::Binary { left, right, .. } => {
                Self::push_value_uses(left, &mut uses);
                Self::push_value_uses(right, &mut uses);
            }
            Instruction::Unary { value, .. } | Instruction::Move { value, .. } => {
                Self::push_value_uses(value, &mut uses);
            }
            Instruction::Call { args, .. } => {
                for arg in args {
                    Self::push_value_uses(arg, &mut uses);
                }
            }
        }
        uses
    }

    fn push_value_uses(value: &Value, uses: &mut Vec<String>) {
        if let Value::Local(name) = value {
            uses.push(name.clone());
        }
    }

    fn eliminate_in_block(block: &mut BasicBlock, live_in: &HashMap<usize, HashSet<String>>) {
        // Live set at the end of this block's instructions.
        let mut live = HashSet::new();
        for succ in Self::successors(&block.terminator) {
            live.extend(live_in[&succ].iter().cloned());
        }
        for used in Self::terminator_uses(&block.terminator) {
            live.insert(used);
        }

        let mut kept = Vec::with_capacity(block.instructions.len());
        for inst in block.instructions.iter().rev() {
            // Calls may have side effects; always keep them.
            let keep = matches!(inst, Instruction::Call { .. })
                || Self::dest_of(inst).is_some_and(|dest| live.contains(&dest));

            if keep {
                for used in Self::used_by(inst) {
                    live.insert(used);
                }
                // Keeping an instruction kills its destination.
                if let Some(dest) = Self::dest_of(inst) {
                    live.remove(&dest);
                }
                kept.push(inst.clone());
            }
        }
        kept.reverse();
        block.instructions = kept;
    }

    fn terminator_uses(terminator: &Terminator) -> Vec<String> {
        match terminator {
            Terminator::Return(Some(value)) => {
                let mut uses = Vec::new();
                Self::push_value_uses(value, &mut uses);
                uses
            }
            Terminator::Branch { condition, .. } => {
                let mut uses = Vec::new();
                Self::push_value_uses(condition, &mut uses);
                uses
            }
            _ => vec![],
        }
    }
}
