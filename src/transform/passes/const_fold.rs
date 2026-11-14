// SPDX-License-Identifier: MIT

//! Constant folding pass.
//!
//! Evaluates constant expressions at compile time and replaces them with
//! their computed values. Within each block, constants assigned to temps are
//! propagated into later instructions, so `2 * 3 + 4` folds all the way down
//! to `10`. Branch terminators whose condition became a constant are turned
//! into unconditional jumps, and blocks that became unreachable as a result
//! are removed.

use crate::transform::ir::*;
use std::collections::HashMap;

pub struct ConstFold;

impl ConstFold {
    pub fn run(module: &mut Module) {
        for func in &mut module.functions {
            for block in &mut func.body {
                Self::fold_block(block);
            }
            Self::fold_constant_branches(func);
            Self::remove_unreachable_blocks(func);
        }
    }

    fn fold_block(block: &mut BasicBlock) {
        // Temps known to hold constants in this block, for propagation.
        let mut constants: HashMap<String, Value> = HashMap::new();

        for inst in &mut block.instructions {
            match inst {
                Instruction::Binary {
                    dest,
                    left,
                    op,
                    right,
                    ty,
                } => {
                    // Propagate known constants into the operands.
                    Self::propagate(left, &constants);
                    Self::propagate(right, &constants);

                    let dest = dest.clone();
                    let folded = match (Self::as_const_int(left), Self::as_const_int(right)) {
                        (Some(l), Some(r)) => match op {
                            BinOp::Add => Some(l.wrapping_add(r)),
                            BinOp::Sub => Some(l.wrapping_sub(r)),
                            BinOp::Mul => Some(l.wrapping_mul(r)),
                            BinOp::Div if r != 0 => Some(l / r),
                            BinOp::Mod if r != 0 => Some(l % r),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(result) = folded {
                        *inst = Instruction::Move {
                            dest: dest.clone(),
                            value: Value::ConstInt(result),
                            ty: ty.clone(),
                        };
                    }

                    if let Some(v) = Self::const_value(inst) {
                        constants.insert(dest, v);
                    }
                }
                Instruction::Move { dest, value, .. } => {
                    Self::propagate(value, &constants);
                    let dest = dest.clone();
                    if let Some(v) = Self::const_value(inst) {
                        constants.insert(dest, v);
                    }
                }
                Instruction::Unary {
                    dest,
                    op,
                    value,
                    ty,
                } => {
                    Self::propagate(value, &constants);
                    let dest = dest.clone();
                    if let Some(v) = Self::as_const_int(value) {
                        let result = match op {
                            UnaryOp::Neg => Value::ConstInt(v.wrapping_neg()),
                            UnaryOp::Not => Value::ConstBool(v == 0),
                        };
                        *inst = Instruction::Move {
                            dest: dest.clone(),
                            value: result.clone(),
                            ty: ty.clone(),
                        };
                        constants.insert(dest, result);
                    }
                }
                // Calls can have side effects; leave them alone.
                Instruction::Call { .. } => {}
            }
        }

        // The branch condition may have become a constant through propagation.
        if let Terminator::Branch { condition, .. } = &mut block.terminator
            && let Value::Local(name) = condition
            && let Some(v) = constants.get(name)
        {
            *condition = v.clone();
        }
    }

    /// Replaces `Branch` terminators with constant conditions by
    /// unconditional jumps to the taken side.
    fn fold_constant_branches(func: &mut Function) {
        for block in &mut func.body {
            if let Terminator::Branch {
                condition: Value::ConstBool(true),
                then_block,
                ..
            } = block.terminator
            {
                block.terminator = Terminator::Jump(then_block);
            } else if let Terminator::Branch {
                condition: Value::ConstBool(false),
                else_block,
                ..
            } = block.terminator
            {
                block.terminator = Terminator::Jump(else_block);
            }
        }
    }

    /// Drops blocks that can no longer be reached from the entry block, and
    /// renumbers the remaining ones consecutively.
    fn remove_unreachable_blocks(func: &mut Function) {
        if func.body.is_empty() {
            return;
        }

        // Mark reachable blocks by walking the control-flow graph.
        let mut reachable = vec![false; func.body.len()];
        let mut stack = vec![0usize];
        reachable[0] = true;
        while let Some(id) = stack.pop() {
            match &func.body[id].terminator {
                Terminator::Return(_) => {}
                Terminator::Jump(target) => {
                    if !reachable[*target] {
                        reachable[*target] = true;
                        stack.push(*target);
                    }
                }
                Terminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    for target in [then_block, else_block] {
                        if !reachable[*target] {
                            reachable[*target] = true;
                            stack.push(*target);
                        }
                    }
                }
            }
        }

        // Renumber reachable blocks in their original order.
        let mut remap: HashMap<usize, usize> = HashMap::new();
        let mut kept = Vec::new();
        for (id, mut block) in func.body.drain(..).enumerate() {
            if reachable[id] {
                remap.insert(id, kept.len());
                block.id = kept.len();
                kept.push(block);
            }
        }

        for block in &mut kept {
            match &mut block.terminator {
                Terminator::Jump(target) => *target = remap[target],
                Terminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    *then_block = remap[&*then_block];
                    *else_block = remap[&*else_block];
                }
                Terminator::Return(_) => {}
            }
        }

        func.body = kept;
    }

    /// Replaces a local operand with its known constant, if any.
    fn propagate(value: &mut Value, constants: &HashMap<String, Value>) {
        if let Value::Local(name) = value
            && let Some(v) = constants.get(name)
        {
            *value = v.clone();
        }
    }

    /// If the instruction now binds its dest to a constant, returns it.
    fn const_value(inst: &Instruction) -> Option<Value> {
        match inst {
            Instruction::Move { value, .. } => match value {
                Value::ConstInt(_) | Value::ConstBool(_) | Value::ConstString(_) => {
                    Some(value.clone())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn as_const_int(value: &Value) -> Option<i64> {
        match value {
            Value::ConstInt(v) => Some(*v),
            _ => None,
        }
    }
}
