// SPDX-License-Identifier: MIT

//! Constant folding pass.
//!
//! Evaluates constant expressions at compile time and replaces them with
//! their computed values. Within each block, constants assigned to temps are
//! also propagated into later instructions, so `2 * 3 + 4` folds all the way
//! down to `10`.

use crate::transform::ir::*;
use std::collections::HashMap;

pub struct ConstFold;

impl ConstFold {
    pub fn run(module: &mut Module) {
        for func in &mut module.functions {
            for block in &mut func.body {
                Self::fold_block(block);
            }
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
