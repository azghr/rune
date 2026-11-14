//! Constant folding pass.
//!
//! Evaluates constant expressions at compile time and replaces them
//! with their computed values.

use crate::transform::ir::*;

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
        for inst in &mut block.instructions {
            if let Instruction::Binary {
                dest,
                left,
                op,
                right,
                ty,
            } = inst
            {
                if let (Some(l), Some(r)) = (Self::as_const_int(left), Self::as_const_int(right)) {
                    let result = match op {
                        BinOp::Add => l + r,
                        BinOp::Sub => l - r,
                        BinOp::Mul => l * r,
                        BinOp::Div if r != 0 => l / r,
                        BinOp::Mod if r != 0 => l % r,
                        _ => continue,
                    };
                    *inst = Instruction::Move {
                        dest: dest.clone(),
                        value: Value::ConstInt(result),
                        ty: ty.clone(),
                    };
                }
            }
        }
    }

    fn as_const_int(value: &Value) -> Option<i64> {
        match value {
            Value::ConstInt(v) => Some(*v),
            _ => None,
        }
    }
}
